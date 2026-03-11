# Elixir Syntax Highlighting Test
# A GenServer-based rate limiter with ETS caching.

defmodule RateLimiter do
  @moduledoc """
  A sliding window rate limiter backed by ETS.

  ## Usage

      {:ok, _pid} = RateLimiter.start_link(
        max_requests: 100,
        window_ms: 60_000
      )

      case RateLimiter.check("user:123") do
        :ok -> handle_request()
        {:error, :rate_limited, retry_after} -> send_429(retry_after)
      end
  """

  use GenServer
  require Logger

  @default_max_requests 100
  @default_window_ms 60_000
  @cleanup_interval_ms 30_000
  @table_name :rate_limiter_buckets

  # Type specs
  @type client_id :: String.t()
  @type timestamp :: integer()
  @type bucket :: {client_id(), [timestamp()]}

  defstruct [:max_requests, :window_ms, :table, :cleanup_ref]

  # Public API

  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @spec check(client_id()) :: :ok | {:error, :rate_limited, non_neg_integer()}
  def check(client_id) when is_binary(client_id) do
    GenServer.call(__MODULE__, {:check, client_id})
  end

  @spec reset(client_id()) :: :ok
  def reset(client_id), do: GenServer.cast(__MODULE__, {:reset, client_id})

  @spec stats() :: map()
  def stats, do: GenServer.call(__MODULE__, :stats)

  # GenServer callbacks

  @impl true
  def init(opts) do
    table = :ets.new(@table_name, [:set, :public, read_concurrency: true])

    state = %__MODULE__{
      max_requests: Keyword.get(opts, :max_requests, @default_max_requests),
      window_ms: Keyword.get(opts, :window_ms, @default_window_ms),
      table: table,
      cleanup_ref: schedule_cleanup()
    }

    Logger.info("RateLimiter started: #{inspect(state.max_requests)} req/#{state.window_ms}ms")
    {:ok, state}
  end

  @impl true
  def handle_call({:check, client_id}, _from, state) do
    now = System.monotonic_time(:millisecond)
    cutoff = now - state.window_ms

    timestamps =
      case :ets.lookup(state.table, client_id) do
        [{^client_id, ts_list}] -> ts_list
        [] -> []
      end

    # Filter to current window
    current = Enum.filter(timestamps, &(&1 > cutoff))
    count = length(current)

    result =
      if count < state.max_requests do
        :ets.insert(state.table, {client_id, [now | current]})
        :ok
      else
        oldest = Enum.min(current)
        retry_after = oldest + state.window_ms - now
        {:error, :rate_limited, max(retry_after, 0)}
      end

    {:reply, result, state}
  end

  @impl true
  def handle_call(:stats, _from, state) do
    now = System.monotonic_time(:millisecond)
    cutoff = now - state.window_ms

    stats =
      :ets.tab2list(state.table)
      |> Enum.map(fn {client_id, timestamps} ->
        active = Enum.count(timestamps, &(&1 > cutoff))
        {client_id, active}
      end)
      |> Enum.into(%{})

    total = stats |> Map.values() |> Enum.sum()

    result = %{
      clients: map_size(stats),
      total_requests: total,
      per_client: stats,
      config: %{
        max_requests: state.max_requests,
        window_ms: state.window_ms
      }
    }

    {:reply, result, state}
  end

  @impl true
  def handle_cast({:reset, client_id}, state) do
    :ets.delete(state.table, client_id)
    {:noreply, state}
  end

  @impl true
  def handle_info(:cleanup, state) do
    now = System.monotonic_time(:millisecond)
    cutoff = now - state.window_ms

    expired =
      :ets.tab2list(state.table)
      |> Enum.reduce(0, fn {client_id, timestamps}, acc ->
        active = Enum.filter(timestamps, &(&1 > cutoff))

        if Enum.empty?(active) do
          :ets.delete(state.table, client_id)
          acc + 1
        else
          :ets.insert(state.table, {client_id, active})
          acc
        end
      end)

    if expired > 0 do
      Logger.debug("Cleaned up #{expired} expired rate limit buckets")
    end

    {:noreply, %{state | cleanup_ref: schedule_cleanup()}}
  end

  # Private helpers

  defp schedule_cleanup do
    Process.send_after(self(), :cleanup, @cleanup_interval_ms)
  end
end

# Pipeline and pattern matching examples
defmodule DataPipeline do
  @moduledoc false

  @type record :: %{required(String.t()) => term()}

  @doc "Process a batch of records through the pipeline."
  @spec process(Enumerable.t()) :: {:ok, [record()]} | {:error, term()}
  def process(records) do
    records
    |> Stream.map(&normalize/1)
    |> Stream.filter(&valid?/1)
    |> Stream.map(&enrich/1)
    |> Stream.chunk_every(50)
    |> Stream.flat_map(&process_batch/1)
    |> Enum.to_list()
    |> then(&{:ok, &1})
  rescue
    e in RuntimeError -> {:error, e.message}
  end

  defp normalize(%{"name" => name, "email" => email} = record) do
    %{record | "name" => String.trim(name), "email" => String.downcase(email)}
  end

  defp normalize(record), do: record

  defp valid?(%{"email" => email}) when is_binary(email) do
    String.match?(email, ~r/^[\w.+-]+@[\w-]+\.[\w.]+$/)
  end

  defp valid?(_), do: false

  defp enrich(record) do
    record
    |> Map.put("processed_at", DateTime.utc_now())
    |> Map.put("hash", :crypto.hash(:sha256, record["email"]) |> Base.encode16(case: :lower))
    |> Map.update("tags", [], &Enum.uniq/1)
  end

  defp process_batch(batch) do
    # Simulate async processing
    batch
    |> Task.async_stream(
      fn record ->
        Process.sleep(10)
        Map.put(record, "status", "processed")
      end,
      max_concurrency: System.schedulers_online(),
      timeout: 5_000
    )
    |> Enum.map(fn
      {:ok, result} -> result
      {:exit, reason} ->
        Logger.warning("Batch item failed: #{inspect(reason)}")
        nil
    end)
    |> Enum.reject(&is_nil/1)
  end
end

# Sigils, guards, and protocol implementation
defmodule Token do
  @enforce_keys [:type, :value]
  defstruct [:type, :value, :line, :column, metadata: %{}]

  @type t :: %__MODULE__{
    type: atom(),
    value: String.t(),
    line: non_neg_integer() | nil,
    column: non_neg_integer() | nil,
    metadata: map()
  }

  # Guard clause pattern matching
  def keyword?(%__MODULE__{type: type})
      when type in ~w(def defmodule defp if else case cond fn do end)a do
    true
  end

  def keyword?(_), do: false

  def heredoc_example do
    ~S"""
    This is a sigil string with no interpolation.
    Special chars like \n and #{expr} are literal.
    """
  end

  def regex_example do
    patterns = [
      ~r/\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b/,
      ~r/^[A-Z][a-zA-Z0-9]*$/,
      ~r/(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})/
    ]

    Enum.map(patterns, &Regex.source/1)
  end
end

defimpl String.Chars, for: Token do
  def to_string(%Token{type: type, value: value, line: line}) do
    "#{type}(#{inspect(value)})#{if line, do: ":#{line}", else: ""}"
  end
end

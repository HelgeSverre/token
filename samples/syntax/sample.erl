%%% Erlang Syntax Highlighting Test
%%% A gen_server task manager with OTP supervision and message passing.

-module(task_manager).
-behaviour(gen_server).

%% API exports
-export([
    start_link/0,
    start_link/1,
    create/2,
    create/3,
    get/1,
    update_status/2,
    delete/1,
    list_all/0,
    list_by_status/1,
    list_by_tag/1,
    stats/0,
    stop/0
]).

%% gen_server callbacks
-export([
    init/1,
    handle_call/3,
    handle_cast/2,
    handle_info/2,
    terminate/2,
    code_change/3
]).

%% Types
-type priority() :: low | medium | high | critical.
-type status() :: open | in_progress | done | cancelled.
-type task_id() :: pos_integer().

-record(task, {
    id          :: task_id(),
    title       :: binary(),
    description :: binary(),
    status      :: status(),
    priority    :: priority(),
    tags        :: [binary()],
    created_at  :: calendar:datetime(),
    updated_at  :: calendar:datetime()
}).

-record(state, {
    tasks   :: #{task_id() => #task{}},
    next_id :: task_id(),
    config  :: map()
}).

-type task() :: #task{}.
-type state() :: #state{}.

-define(SERVER, ?MODULE).
-define(DEFAULT_CONFIG, #{
    max_tasks => 10000,
    cleanup_interval => 3600000  %% 1 hour in milliseconds
}).

%% ============================================================
%% API Functions
%% ============================================================

-spec start_link() -> {ok, pid()} | {error, term()}.
start_link() ->
    start_link(?DEFAULT_CONFIG).

-spec start_link(map()) -> {ok, pid()} | {error, term()}.
start_link(Config) ->
    gen_server:start_link({local, ?SERVER}, ?MODULE, Config, []).

-spec create(binary(), priority()) -> {ok, task()} | {error, term()}.
create(Title, Priority) ->
    create(Title, Priority, []).

-spec create(binary(), priority(), [binary()]) -> {ok, task()} | {error, term()}.
create(Title, Priority, Tags) when is_binary(Title),
                                    is_atom(Priority),
                                    is_list(Tags) ->
    gen_server:call(?SERVER, {create, Title, Priority, Tags}).

-spec get(task_id()) -> {ok, task()} | {error, not_found}.
get(Id) when is_integer(Id) ->
    gen_server:call(?SERVER, {get, Id}).

-spec update_status(task_id(), status()) -> {ok, task()} | {error, term()}.
update_status(Id, NewStatus) when is_integer(Id), is_atom(NewStatus) ->
    gen_server:call(?SERVER, {update_status, Id, NewStatus}).

-spec delete(task_id()) -> ok | {error, not_found}.
delete(Id) when is_integer(Id) ->
    gen_server:call(?SERVER, {delete, Id}).

-spec list_all() -> [task()].
list_all() ->
    gen_server:call(?SERVER, list_all).

-spec list_by_status(status()) -> [task()].
list_by_status(Status) when is_atom(Status) ->
    gen_server:call(?SERVER, {list_by_status, Status}).

-spec list_by_tag(binary()) -> [task()].
list_by_tag(Tag) when is_binary(Tag) ->
    gen_server:call(?SERVER, {list_by_tag, Tag}).

-spec stats() -> map().
stats() ->
    gen_server:call(?SERVER, stats).

-spec stop() -> ok.
stop() ->
    gen_server:stop(?SERVER).

%% ============================================================
%% gen_server Callbacks
%% ============================================================

init(Config) ->
    process_flag(trap_exit, true),
    MergedConfig = maps:merge(?DEFAULT_CONFIG, Config),

    %% Schedule periodic cleanup
    CleanupInterval = maps:get(cleanup_interval, MergedConfig),
    erlang:send_after(CleanupInterval, self(), cleanup),

    io:format("Task manager started with config: ~p~n", [MergedConfig]),

    {ok, #state{
        tasks = #{},
        next_id = 1,
        config = MergedConfig
    }}.

handle_call({create, Title, Priority, Tags}, _From, State) ->
    #state{tasks = Tasks, next_id = NextId, config = Config} = State,

    MaxTasks = maps:get(max_tasks, Config),
    case maps:size(Tasks) >= MaxTasks of
        true ->
            {reply, {error, max_tasks_reached}, State};
        false ->
            Now = calendar:universal_time(),
            Task = #task{
                id = NextId,
                title = Title,
                description = <<>>,
                status = open,
                priority = Priority,
                tags = Tags,
                created_at = Now,
                updated_at = Now
            },
            NewTasks = Tasks#{NextId => Task},
            NewState = State#state{tasks = NewTasks, next_id = NextId + 1},
            {reply, {ok, Task}, NewState}
    end;

handle_call({get, Id}, _From, #state{tasks = Tasks} = State) ->
    case maps:find(Id, Tasks) of
        {ok, Task} -> {reply, {ok, Task}, State};
        error      -> {reply, {error, not_found}, State}
    end;

handle_call({update_status, Id, NewStatus}, _From, State) ->
    #state{tasks = Tasks} = State,
    case maps:find(Id, Tasks) of
        {ok, Task} ->
            case validate_transition(Task#task.status, NewStatus) of
                ok ->
                    Updated = Task#task{
                        status = NewStatus,
                        updated_at = calendar:universal_time()
                    },
                    NewTasks = Tasks#{Id := Updated},
                    {reply, {ok, Updated}, State#state{tasks = NewTasks}};
                {error, Reason} ->
                    {reply, {error, Reason}, State}
            end;
        error ->
            {reply, {error, not_found}, State}
    end;

handle_call({delete, Id}, _From, #state{tasks = Tasks} = State) ->
    case maps:is_key(Id, Tasks) of
        true ->
            NewTasks = maps:remove(Id, Tasks),
            {reply, ok, State#state{tasks = NewTasks}};
        false ->
            {reply, {error, not_found}, State}
    end;

handle_call(list_all, _From, #state{tasks = Tasks} = State) ->
    Sorted = lists:sort(
        fun(A, B) -> priority_value(A#task.priority) >= priority_value(B#task.priority) end,
        maps:values(Tasks)
    ),
    {reply, Sorted, State};

handle_call({list_by_status, Status}, _From, #state{tasks = Tasks} = State) ->
    Filtered = [T || T <- maps:values(Tasks), T#task.status =:= Status],
    {reply, Filtered, State};

handle_call({list_by_tag, Tag}, _From, #state{tasks = Tasks} = State) ->
    Filtered = [T || T <- maps:values(Tasks), lists:member(Tag, T#task.tags)],
    {reply, Filtered, State};

handle_call(stats, _From, #state{tasks = Tasks} = State) ->
    AllTasks = maps:values(Tasks),
    Total = length(AllTasks),

    ByStatus = lists:foldl(
        fun(T, Acc) ->
            S = T#task.status,
            Acc#{S => maps:get(S, Acc, 0) + 1}
        end,
        #{},
        AllTasks
    ),

    ByPriority = lists:foldl(
        fun(T, Acc) ->
            P = T#task.priority,
            Acc#{P => maps:get(P, Acc, 0) + 1}
        end,
        #{},
        AllTasks
    ),

    DoneCount = maps:get(done, ByStatus, 0),
    CompletionRate = case Total of
        0 -> 0.0;
        _ -> DoneCount / Total * 100.0
    end,

    Stats = #{
        total => Total,
        by_status => ByStatus,
        by_priority => ByPriority,
        completion_rate => CompletionRate
    },
    {reply, Stats, State}.

handle_cast(_Msg, State) ->
    {noreply, State}.

handle_info(cleanup, #state{tasks = Tasks, config = Config} = State) ->
    %% Remove cancelled tasks older than 24 hours
    Now = calendar:universal_time(),
    NowSecs = calendar:datetime_to_gregorian_seconds(Now),
    DayInSecs = 86400,

    NewTasks = maps:filter(
        fun(_Id, Task) ->
            case Task#task.status of
                cancelled ->
                    TaskSecs = calendar:datetime_to_gregorian_seconds(Task#task.updated_at),
                    NowSecs - TaskSecs < DayInSecs;
                _ ->
                    true
            end
        end,
        Tasks
    ),

    Removed = maps:size(Tasks) - maps:size(NewTasks),
    case Removed > 0 of
        true  -> io:format("Cleanup: removed ~p cancelled tasks~n", [Removed]);
        false -> ok
    end,

    %% Reschedule
    CleanupInterval = maps:get(cleanup_interval, Config),
    erlang:send_after(CleanupInterval, self(), cleanup),

    {noreply, State#state{tasks = NewTasks}};

handle_info(_Info, State) ->
    {noreply, State}.

terminate(Reason, _State) ->
    io:format("Task manager stopping: ~p~n", [Reason]),
    ok.

code_change(_OldVsn, State, _Extra) ->
    {ok, State}.

%% ============================================================
%% Internal Functions
%% ============================================================

-spec validate_transition(status(), status()) -> ok | {error, invalid_transition}.
validate_transition(Current, New) ->
    Valid = #{
        open        => [in_progress, cancelled],
        in_progress => [open, done, cancelled],
        done        => [open],
        cancelled   => [open]
    },
    case maps:find(Current, Valid) of
        {ok, Allowed} ->
            case lists:member(New, Allowed) of
                true  -> ok;
                false -> {error, invalid_transition}
            end;
        error ->
            {error, invalid_transition}
    end.

-spec priority_value(priority()) -> non_neg_integer().
priority_value(low)      -> 0;
priority_value(medium)   -> 1;
priority_value(high)     -> 2;
priority_value(critical) -> 3.

-spec format_task(task()) -> iolist().
format_task(#task{id = Id, title = Title, status = Status,
                  priority = Priority, tags = Tags}) ->
    Icon = case Status of
        open        -> "[ ]";
        in_progress -> "[~]";
        done        -> "[x]";
        cancelled   -> "[-]"
    end,
    Prio = case Priority of
        low      -> " ";
        medium   -> "!";
        high     -> "!!";
        critical -> "!!!"
    end,
    TagStr = case Tags of
        [] -> "";
        _  -> io_lib:format(" [~s]", [lists:join(<<", ">>, Tags)])
    end,
    io_lib:format("#~B ~s ~s ~s~s", [Id, Icon, Prio, Title, TagStr]).

/// C# Syntax Highlighting Test
/// An async task scheduler with LINQ, records, and pattern matching.

using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace TaskScheduler;

// Record types (C# 10+)
public record struct TaskId(Guid Value)
{
    public static TaskId New() => new(Guid.NewGuid());
    public override string ToString() => Value.ToString()[..8];
}

public record TaskDefinition(
    string Name,
    Func<CancellationToken, Task<TaskResult>> Execute,
    TimeSpan? Timeout = null,
    int MaxRetries = 0,
    IReadOnlyList<string>? Tags = null
);

public record TaskResult(
    TaskId Id,
    string Name,
    TaskStatus Status,
    TimeSpan Duration,
    string? Error = null,
    object? Output = null
);

// Enum with attributes
public enum TaskStatus
{
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut
}

// Interface with default implementation
public interface ITaskObserver
{
    void OnStarted(TaskId id, string name) { }
    void OnCompleted(TaskResult result) { }
    void OnError(TaskId id, Exception ex) { }

    void OnBatchCompleted(IReadOnlyList<TaskResult> results)
    {
        var summary = results
            .GroupBy(r => r.Status)
            .Select(g => $"{g.Key}: {g.Count()}")
            .Aggregate((a, b) => $"{a}, {b}");
        Console.WriteLine($"Batch complete: {summary}");
    }
}

// Generic constraint and where clause
public class PriorityQueue<T> where T : IComparable<T>
{
    private readonly List<(T Item, int Priority)> _items = new();
    private readonly object _lock = new();

    public int Count
    {
        get { lock (_lock) return _items.Count; }
    }

    public void Enqueue(T item, int priority)
    {
        lock (_lock)
        {
            _items.Add((item, priority));
            _items.Sort((a, b) => b.Priority.CompareTo(a.Priority));
        }
    }

    public T? Dequeue()
    {
        lock (_lock)
        {
            if (_items.Count == 0) return default;
            var item = _items[0].Item;
            _items.RemoveAt(0);
            return item;
        }
    }
}

// Main scheduler class
public sealed class Scheduler : IAsyncDisposable
{
    private readonly ConcurrentDictionary<TaskId, TaskEntry> _tasks = new();
    private readonly SemaphoreSlim _semaphore;
    private readonly List<ITaskObserver> _observers = new();
    private readonly CancellationTokenSource _cts = new();
    private readonly int _maxConcurrency;

    private record TaskEntry(
        TaskId Id,
        TaskDefinition Definition,
        DateTime QueuedAt,
        int Attempt = 0
    );

    public Scheduler(int maxConcurrency = 4)
    {
        _maxConcurrency = maxConcurrency;
        _semaphore = new SemaphoreSlim(maxConcurrency, maxConcurrency);
    }

    public void AddObserver(ITaskObserver observer) => _observers.Add(observer);

    public TaskId Schedule(TaskDefinition definition)
    {
        var id = TaskId.New();
        var entry = new TaskEntry(id, definition, DateTime.UtcNow);
        _tasks.TryAdd(id, entry);
        return id;
    }

    public async Task<TaskResult[]> RunAllAsync(CancellationToken ct = default)
    {
        using var linkedCts = CancellationTokenSource.CreateLinkedTokenSource(ct, _cts.Token);
        var token = linkedCts.Token;

        var tasks = _tasks.Values
            .OrderBy(e => e.QueuedAt)
            .Select(entry => RunWithSemaphore(entry, token))
            .ToArray();

        var results = await Task.WhenAll(tasks);

        foreach (var observer in _observers)
            observer.OnBatchCompleted(results);

        return results;
    }

    private async Task<TaskResult> RunWithSemaphore(TaskEntry entry, CancellationToken ct)
    {
        await _semaphore.WaitAsync(ct);
        try
        {
            return await ExecuteWithRetry(entry, ct);
        }
        finally
        {
            _semaphore.Release();
        }
    }

    private async Task<TaskResult> ExecuteWithRetry(TaskEntry entry, CancellationToken ct)
    {
        var def = entry.Definition;
        var sw = System.Diagnostics.Stopwatch.StartNew();

        for (int attempt = 0; attempt <= def.MaxRetries; attempt++)
        {
            try
            {
                foreach (var obs in _observers)
                    obs.OnStarted(entry.Id, def.Name);

                using var timeoutCts = def.Timeout.HasValue
                    ? new CancellationTokenSource(def.Timeout.Value)
                    : new CancellationTokenSource();

                using var linked = CancellationTokenSource
                    .CreateLinkedTokenSource(ct, timeoutCts.Token);

                var result = await def.Execute(linked.Token);
                sw.Stop();

                var finalResult = result with { Duration = sw.Elapsed };
                foreach (var obs in _observers)
                    obs.OnCompleted(finalResult);

                return finalResult;
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested)
            {
                sw.Stop();
                return new TaskResult(entry.Id, def.Name, TaskStatus.Cancelled, sw.Elapsed);
            }
            catch (OperationCanceledException)
            {
                sw.Stop();
                return new TaskResult(entry.Id, def.Name, TaskStatus.TimedOut, sw.Elapsed,
                    $"Timed out after {def.Timeout}");
            }
            catch (Exception ex) when (attempt < def.MaxRetries)
            {
                foreach (var obs in _observers)
                    obs.OnError(entry.Id, ex);

                var delay = TimeSpan.FromMilliseconds(Math.Pow(2, attempt) * 100);
                await Task.Delay(delay, ct);
            }
            catch (Exception ex)
            {
                sw.Stop();
                return new TaskResult(entry.Id, def.Name, TaskStatus.Failed, sw.Elapsed,
                    ex.Message);
            }
        }

        // Should not reach here
        sw.Stop();
        return new TaskResult(entry.Id, def.Name, TaskStatus.Failed, sw.Elapsed);
    }

    // Pattern matching on task results
    public static string Summarize(TaskResult result) => result switch
    {
        { Status: TaskStatus.Completed, Duration.TotalSeconds: < 1 } =>
            $"✓ {result.Name} (fast: {result.Duration.TotalMilliseconds:F0}ms)",

        { Status: TaskStatus.Completed } =>
            $"✓ {result.Name} ({result.Duration.TotalSeconds:F1}s)",

        { Status: TaskStatus.Failed, Error: string err } =>
            $"✗ {result.Name}: {err}",

        { Status: TaskStatus.Cancelled } =>
            $"⊘ {result.Name} cancelled",

        { Status: TaskStatus.TimedOut, Error: string err } =>
            $"⏱ {result.Name}: {err}",

        _ => $"? {result.Name}: {result.Status}"
    };

    // LINQ query expression
    public IEnumerable<(string Tag, double AvgMs)> GetTagStats(TaskResult[] results)
    {
        return from result in results
               where result.Status == TaskStatus.Completed
               from tag in _tasks.Values
                   .Where(t => t.Id == result.Id)
                   .SelectMany(t => t.Definition.Tags ?? Array.Empty<string>())
               group result.Duration.TotalMilliseconds by tag into g
               orderby g.Average() descending
               select (Tag: g.Key, AvgMs: g.Average());
    }

    public async ValueTask DisposeAsync()
    {
        _cts.Cancel();
        _cts.Dispose();
        _semaphore.Dispose();
    }
}

// Extension methods
public static class TaskResultExtensions
{
    public static bool IsSuccess(this TaskResult result) =>
        result.Status == TaskStatus.Completed;

    public static T? OutputAs<T>(this TaskResult result) where T : class =>
        result.Output as T;

    public static string ToJson(this TaskResult result) =>
        JsonSerializer.Serialize(result, new JsonSerializerOptions
        {
            WriteIndented = true,
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase
        });
}

// Entry point with top-level statements style
public static class Program
{
    public static async Task Main(string[] args)
    {
        await using var scheduler = new Scheduler(maxConcurrency: 3);

        scheduler.AddObserver(new ConsoleObserver());

        var tasks = Enumerable.Range(1, 5).Select(i =>
            new TaskDefinition(
                Name: $"Task-{i}",
                Execute: async ct =>
                {
                    await Task.Delay(Random.Shared.Next(100, 500), ct);
                    return new TaskResult(
                        TaskId.New(), $"Task-{i}", TaskStatus.Completed,
                        TimeSpan.Zero, Output: $"Result from task {i}"
                    );
                },
                Timeout: TimeSpan.FromSeconds(5),
                MaxRetries: 2,
                Tags: new[] { i % 2 == 0 ? "even" : "odd", "batch-1" }
            )
        ).ToList();

        foreach (var task in tasks)
            scheduler.Schedule(task);

        var results = await scheduler.RunAllAsync();

        foreach (var result in results)
            Console.WriteLine(Scheduler.Summarize(result));
    }
}

file class ConsoleObserver : ITaskObserver
{
    public void OnStarted(TaskId id, string name) =>
        Console.WriteLine($"  → Starting {name} [{id}]");

    public void OnCompleted(TaskResult result) =>
        Console.WriteLine($"  ✓ {result.Name} in {result.Duration.TotalMilliseconds:F0}ms");

    public void OnError(TaskId id, Exception ex) =>
        Console.Error.WriteLine($"  ✗ [{id}] {ex.Message}");
}

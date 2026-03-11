/**
 * D Language Syntax Highlighting Test
 * A compile-time JSON parser with metaprogramming, ranges, and CTFE.
 */

module sample;

import std.stdio;
import std.string;
import std.conv;
import std.algorithm;
import std.range;
import std.array;
import std.format;
import std.traits;
import std.typecons;
import std.exception;
import core.time;

// ============================================================
// Compile-time string processing
// ============================================================

/// Generate a struct from field names at compile time
mixin template GenerateStruct(string name, Fields...) {
    mixin("struct " ~ name ~ " {");
    static foreach (field; Fields) {
        mixin(field.type ~ " " ~ field.name ~ ";");
    }
    mixin("}");
}

/// Field descriptor for code generation
struct Field {
    string name;
    string type;
}

// Generate Task struct at compile time
mixin GenerateStruct!("AutoTask",
    Field("id", "int"),
    Field("title", "string"),
    Field("priority", "int"),
);

// ============================================================
// Enum and tagged union
// ============================================================

enum Priority : ubyte {
    low = 0,
    medium = 1,
    high = 2,
    critical = 3,
}

string toString(Priority p) {
    final switch (p) {
        case Priority.low: return "low";
        case Priority.medium: return "medium";
        case Priority.high: return "high";
        case Priority.critical: return "critical";
    }
}

enum Status {
    open,
    inProgress,
    done,
    cancelled,
}

// Algebraic data type using std.typecons
alias JsonValue = Algebraic!(
    typeof(null),
    bool,
    long,
    double,
    string,
    JsonValue[],
    JsonValue[string],
);

// ============================================================
// Task with operator overloading
// ============================================================

struct Task {
    int id;
    string title;
    string description;
    Status status = Status.open;
    Priority priority = Priority.medium;
    string[] tags;
    MonoTime createdAt;

    // Comparison by priority (for sorting)
    int opCmp(ref const Task other) const {
        if (priority != other.priority)
            return (cast(int) other.priority) - (cast(int) priority);
        return id - other.id;
    }

    bool opEquals(ref const Task other) const {
        return id == other.id;
    }

    size_t toHash() const nothrow @safe {
        return hashOf(id);
    }

    // Custom formatting
    void toString(scope void delegate(const(char)[]) sink) const {
        import std.format : formattedWrite;

        string icon = () {
            final switch (status) {
                case Status.open: return "[ ]";
                case Status.inProgress: return "[~]";
                case Status.done: return "[x]";
                case Status.cancelled: return "[-]";
            }
        }();

        string prioMark = () {
            final switch (priority) {
                case Priority.low: return " ";
                case Priority.medium: return "!";
                case Priority.high: return "!!";
                case Priority.critical: return "!!!";
            }
        }();

        formattedWrite(sink, "#%d %s %s %s", id, icon, prioMark, title);

        if (tags.length > 0) {
            formattedWrite(sink, " [%-(%s, %)]", tags);
        }
    }
}

// ============================================================
// Generic container with ranges
// ============================================================

struct SortedArray(T) if (is(typeof(T.init < T.init) : bool)) {
    private T[] data;

    void insert(T item) {
        auto pos = data.assumeSorted.lowerBound(item).length;
        data.insertInPlace(pos, item);
    }

    bool remove(T item) {
        auto r = data.assumeSorted.equalRange(item);
        if (r.empty) return false;
        data = data[0 .. r.front.ptr - data.ptr]
             ~ data[r.front.ptr - data.ptr + r.length .. $];
        return true;
    }

    auto opSlice() const {
        return data[];
    }

    size_t length() const @property {
        return data.length;
    }

    // Range interface
    bool empty() const @property { return data.empty; }
    T front() const @property { return data.front; }
    void popFront() { data.popFront(); }
}

// ============================================================
// Task store with UFCS chains
// ============================================================

class TaskStore {
    private Task[int] tasks;
    private int nextId = 1;

    Task create(string title, Priority priority = Priority.medium, string[] tags = []) {
        auto task = Task(
            nextId, title, "", Status.open, priority,
            tags.dup, MonoTime.currTime
        );
        tasks[nextId] = task;
        nextId++;
        return task;
    }

    bool updateStatus(int id, Status newStatus) {
        if (auto p = id in tasks) {
            p.status = newStatus;
            return true;
        }
        return false;
    }

    bool remove(int id) {
        if (id in tasks) {
            tasks.remove(id);
            return true;
        }
        return false;
    }

    auto all() {
        return tasks.values.sort();
    }

    auto filterByStatus(Status s) {
        return tasks.values
            .filter!(t => t.status == s)
            .array
            .sort();
    }

    auto filterByTag(string tag) {
        return tasks.values
            .filter!(t => t.tags.canFind(tag))
            .array
            .sort();
    }

    size_t count() const @property {
        return tasks.length;
    }
}

// ============================================================
// Statistics with CTFE
// ============================================================

struct Stats {
    size_t total;
    size_t[Status] byStatus;
    size_t[Priority] byPriority;
    double completionRate;
    double avgTagsPerTask;
}

Stats computeStats(Task[] tasks) pure {
    Stats stats;
    stats.total = tasks.length;

    size_t totalTags = 0;
    foreach (task; tasks) {
        stats.byStatus[task.status]++;
        stats.byPriority[task.priority]++;
        totalTags += task.tags.length;
    }

    auto doneCount = stats.byStatus.get(Status.done, 0);
    stats.completionRate = tasks.length > 0
        ? cast(double) doneCount / tasks.length * 100.0
        : 0.0;
    stats.avgTagsPerTask = tasks.length > 0
        ? cast(double) totalTags / tasks.length
        : 0.0;

    return stats;
}

// ============================================================
// Compile-time format string validation
// ============================================================

template FormatString(string fmt) {
    enum validated = () {
        int argCount = 0;
        foreach (i, c; fmt) {
            if (c == '%' && i + 1 < fmt.length && fmt[i + 1] != '%') {
                argCount++;
            }
        }
        return argCount;
    }();

    string apply(Args...)(Args args) if (Args.length == validated) {
        return format(fmt, args);
    }
}

alias taskFmt = FormatString!"#%d [%s] %s";

// ============================================================
// Template constraints and static if
// ============================================================

auto prettyPrint(T)(T value) if (is(T == Task)) {
    return format("%s", value);
}

auto prettyPrint(T)(T value) if (is(T == Stats)) {
    auto lines = appender!string;
    lines ~= format("=== Statistics ===\n");
    lines ~= format("Total: %d\n", value.total);
    lines ~= format("Completion: %.1f%%\n", value.completionRate);

    static if (is(typeof(value.byStatus))) {
        lines ~= "By status:\n";
        foreach (status, count; value.byStatus) {
            lines ~= format("  %s: %d\n", status, count);
        }
    }

    return lines.data;
}

// ============================================================
// Fiber-based concurrency
// ============================================================

import core.thread;

auto processTasksFiber(Task[] tasks) {
    auto results = appender!(string[]);

    foreach (task; tasks) {
        auto fiber = new Fiber({
            // Simulate async work
            results ~= format("Processed: %s", task.title);
            Fiber.yield();
        });

        while (fiber.state != Fiber.State.TERM) {
            fiber.call();
        }
    }

    return results.data;
}

// ============================================================
// Unittest
// ============================================================

unittest {
    auto store = new TaskStore();
    store.create("Test task", Priority.high, ["test"]);
    assert(store.count == 1);

    auto tasks = store.all();
    assert(tasks.length == 1);
    assert(tasks[0].title == "Test task");
    assert(tasks[0].priority == Priority.high);
    assert(tasks[0].tags == ["test"]);

    store.updateStatus(1, Status.done);
    auto stats = computeStats(store.all());
    assert(stats.completionRate == 100.0);
}

unittest {
    auto arr = SortedArray!int();
    arr.insert(3);
    arr.insert(1);
    arr.insert(2);
    assert(arr[].equal([1, 2, 3]));
}

// ============================================================
// Main
// ============================================================

void main() {
    writeln("Task Manager v1.0");
    writeln();

    auto store = new TaskStore();

    store.create("Implement syntax highlighting", Priority.high, ["feature", "syntax"]);
    store.create("Fix cursor blinking", Priority.low, ["bug"]);
    store.create("Add split view", Priority.medium, ["feature", "ui"]);
    store.create("Write documentation", Priority.medium, ["docs"]);
    store.create("Performance profiling", Priority.high, ["perf"]);

    store.updateStatus(1, Status.inProgress);
    store.updateStatus(2, Status.done);

    writeln("All tasks:");
    foreach (task; store.all()) {
        writefln("  %s", task);
    }

    writeln();
    auto stats = computeStats(store.all());
    write(prettyPrint(stats));

    writeln("\nFeature tasks:");
    foreach (task; store.filterByTag("feature")) {
        writefln("  %s", task);
    }
}

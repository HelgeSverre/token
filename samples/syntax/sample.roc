# Roc Syntax Highlighting Test
# A CLI todo application with effects, tags, and pattern matching.

app [main!] {
    pf: platform "https://github.com/roc-lang/basic-cli/releases/download/0.17.0/lZFLstMUCUvd5bjnnpNKIR-Bx2MxLnFLYKVRYWixCIg.tar.br",
    json: "https://github.com/lukewilliamboswell/roc-json/releases/download/0.10.0/KbIfTNbxShRX1A1FgXei1SpO5Jn8sgP6HP6PkKJkwNQ.tar.br",
}

import pf.Stdout
import pf.Stderr
import pf.File
import pf.Path
import pf.Arg
import pf.Env
import json.Json

# ============================================================
# Types
# ============================================================

Priority : [Low, Medium, High, Critical]

Status : [Todo, InProgress, Done, Cancelled]

Task : {
    id : U64,
    title : Str,
    description : Str,
    status : Status,
    priority : Priority,
    tags : List Str,
    createdAt : Str,
}

AppState : {
    tasks : List Task,
    nextId : U64,
    filePath : Str,
}

Command : [
    Add { title : Str, priority : Priority, tags : List Str },
    Done U64,
    Remove U64,
    List { filter : [All, Active, Completed], tag : Result Str [NoTag] },
    Stats,
    Help,
    Unknown Str,
]

# ============================================================
# Priority and Status helpers
# ============================================================

priorityToStr : Priority -> Str
priorityToStr = \priority ->
    when priority is
        Low -> "low"
        Medium -> "medium"
        High -> "high"
        Critical -> "critical"

priorityFromStr : Str -> Result Priority [InvalidPriority]
priorityFromStr = \str ->
    when str is
        "low" -> Ok Low
        "medium" | "med" -> Ok Medium
        "high" -> Ok High
        "critical" | "crit" -> Ok Critical
        _ -> Err InvalidPriority

statusToStr : Status -> Str
statusToStr = \status ->
    when status is
        Todo -> "todo"
        InProgress -> "in-progress"
        Done -> "done"
        Cancelled -> "cancelled"

statusIcon : Status -> Str
statusIcon = \status ->
    when status is
        Todo -> "[ ]"
        InProgress -> "[~]"
        Done -> "[x]"
        Cancelled -> "[-]"

priorityIcon : Priority -> Str
priorityIcon = \priority ->
    when priority is
        Low -> " "
        Medium -> "!"
        High -> "!!"
        Critical -> "!!!"

# ============================================================
# Task operations (pure functions)
# ============================================================

createTask : AppState, Str, Priority, List Str -> { state : AppState, task : Task }
createTask = \state, title, priority, tags ->
    task = {
        id: state.nextId,
        title,
        description: "",
        status: Todo,
        priority,
        tags,
        createdAt: "2024-01-01T00:00:00Z",
    }

    newState = { state &
        tasks: List.append state.tasks task,
        nextId: state.nextId + 1,
    }

    { state: newState, task }

completeTask : AppState, U64 -> Result AppState [TaskNotFound]
completeTask = \state, taskId ->
    updatedTasks =
        state.tasks
        |> List.map \task ->
            if task.id == taskId then
                { task & status: Done }
            else
                task

    hasTask = List.any state.tasks \task -> task.id == taskId

    if hasTask then
        Ok { state & tasks: updatedTasks }
    else
        Err TaskNotFound

removeTask : AppState, U64 -> Result AppState [TaskNotFound]
removeTask = \state, taskId ->
    newTasks = List.dropIf state.tasks \task -> task.id == taskId

    if List.len newTasks == List.len state.tasks then
        Err TaskNotFound
    else
        Ok { state & tasks: newTasks }

filterTasks : List Task, [All, Active, Completed], Result Str [NoTag] -> List Task
filterTasks = \tasks, filter, tagFilter ->
    tasks
    |> List.keepIf \task ->
        statusMatch =
            when filter is
                All -> Bool.true
                Active -> task.status != Done && task.status != Cancelled
                Completed -> task.status == Done

        tagMatch =
            when tagFilter is
                Ok tag -> List.contains task.tags tag
                Err NoTag -> Bool.true

        statusMatch && tagMatch
    |> List.sortWith \a, b ->
        pa = priorityToNum a.priority
        pb = priorityToNum b.priority
        if pa > pb then LT
        else if pa < pb then GT
        else EQ

priorityToNum : Priority -> U8
priorityToNum = \p ->
    when p is
        Low -> 0
        Medium -> 1
        High -> 2
        Critical -> 3

# ============================================================
# Statistics
# ============================================================

TaskStats : {
    total : U64,
    active : U64,
    done : U64,
    byPriority : { low : U64, medium : U64, high : U64, critical : U64 },
    topTags : List (Str, U64),
}

computeStats : List Task -> TaskStats
computeStats = \tasks ->
    total = List.len tasks |> Num.toU64
    active = List.countIf tasks \t -> t.status == Todo || t.status == InProgress |> Num.toU64
    done = List.countIf tasks \t -> t.status == Done |> Num.toU64

    countPriority = \priority ->
        List.countIf tasks \t -> t.priority == priority |> Num.toU64

    # Count tags
    tagCounts =
        tasks
        |> List.joinMap \t -> t.tags
        |> List.walk (Dict.empty {}) \dict, tag ->
            Dict.update dict tag \existing ->
                when existing is
                    Ok n -> Ok (n + 1)
                    Err Missing -> Ok 1u64

    topTags =
        tagCounts
        |> Dict.toList
        |> List.sortWith \(_, a), (_, b) ->
            if a > b then LT
            else if a < b then GT
            else EQ
        |> List.takeFirst 5

    {
        total,
        active,
        done,
        byPriority: {
            low: countPriority Low,
            medium: countPriority Medium,
            high: countPriority High,
            critical: countPriority Critical,
        },
        topTags,
    }

# ============================================================
# Display
# ============================================================

formatTask : Task -> Str
formatTask = \task ->
    icon = statusIcon task.status
    prio = priorityIcon task.priority
    tags =
        if List.isEmpty task.tags then
            ""
        else
            tagStr = task.tags |> Str.joinWith ", "
            " [$(tagStr)]"

    "#$(Num.toStr task.id) $(icon) $(prio) $(task.title)$(tags)"

formatStats : TaskStats -> Str
formatStats = \stats ->
    lines = [
        "=== Task Statistics ===",
        "Total:    $(Num.toStr stats.total)",
        "Active:   $(Num.toStr stats.active)",
        "Done:     $(Num.toStr stats.done)",
        "",
        "By priority:",
        "  Low:      $(Num.toStr stats.byPriority.low)",
        "  Medium:   $(Num.toStr stats.byPriority.medium)",
        "  High:     $(Num.toStr stats.byPriority.high)",
        "  Critical: $(Num.toStr stats.byPriority.critical)",
    ]

    tagLines =
        if List.isEmpty stats.topTags then
            []
        else
            header = ["", "Top tags:"]
            tagEntries = List.map stats.topTags \(tag, count) ->
                "  $(tag): $(Num.toStr count)"
            List.concat header tagEntries

    List.concat lines tagLines
    |> Str.joinWith "\n"

# ============================================================
# Main
# ============================================================

main! : {} => Result {} _
main! = \{} ->
    args = Arg.list! {}

    command = parseArgs args

    state = {
        tasks: [],
        nextId: 1,
        filePath: "tasks.json",
    }

    when command is
        Help ->
            Stdout.line! "Usage: todo <command> [options]"
            Stdout.line! ""
            Stdout.line! "Commands:"
            Stdout.line! "  add <title> [-p priority] [-t tag1,tag2]"
            Stdout.line! "  done <id>"
            Stdout.line! "  remove <id>"
            Stdout.line! "  list [--all|--active|--done] [--tag <tag>]"
            Stdout.line! "  stats"

        Add { title, priority, tags } ->
            result = createTask state title priority tags
            Stdout.line! "Created: $(formatTask result.task)"

        List { filter, tag } ->
            filtered = filterTasks state.tasks filter tag
            if List.isEmpty filtered then
                Stdout.line! "No tasks found."
            else
                List.forEach! filtered \task ->
                    Stdout.line! (formatTask task)

        Stats ->
            stats = computeStats state.tasks
            Stdout.line! (formatStats stats)

        Done id ->
            when completeTask state id is
                Ok _newState -> Stdout.line! "Task #$(Num.toStr id) marked as done."
                Err TaskNotFound -> Stderr.line! "Task #$(Num.toStr id) not found."

        Remove id ->
            when removeTask state id is
                Ok _newState -> Stdout.line! "Task #$(Num.toStr id) removed."
                Err TaskNotFound -> Stderr.line! "Task #$(Num.toStr id) not found."

        Unknown cmd ->
            Stderr.line! "Unknown command: $(cmd). Use 'help' for usage."

parseArgs : List Str -> Command
parseArgs = \args ->
    when List.get args 1 is
        Ok "add" ->
            title = List.get args 2 |> Result.withDefault "Untitled"
            Add { title, priority: Medium, tags: [] }
        Ok "done" ->
            id = List.get args 2 |> Result.try Str.toU64 |> Result.withDefault 0
            Done id
        Ok "remove" ->
            id = List.get args 2 |> Result.try Str.toU64 |> Result.withDefault 0
            Remove id
        Ok "list" -> List { filter: All, tag: Err NoTag }
        Ok "stats" -> Stats
        Ok "help" | Ok "--help" | Ok "-h" -> Help
        Ok other -> Unknown other
        Err _ -> Help

#= Julia Syntax Highlighting Test
   A scientific computing pipeline with multiple dispatch,
   Unicode operators, and metaprogramming.
=#

module TaskAnalytics

using Statistics
using LinearAlgebra
using Printf
using Dates
using Random

export Task, Priority, Status
export create_task, complete_task, compute_stats, simulate_workload

# ============================================================
# Types with parametric polymorphism
# ============================================================

@enum Priority low=0 medium=1 high=2 critical=3
@enum Status open=0 in_progress=1 done=2 cancelled=3

"""
    Task{T}

A task with metadata of type `T`.
"""
struct Task{T}
    id::Int
    title::String
    description::String
    status::Status
    priority::Priority
    tags::Vector{String}
    metadata::T
    created_at::DateTime
end

# Outer constructor with defaults
function Task(id::Int, title::String;
              description::String = "",
              priority::Priority = medium,
              tags::Vector{String} = String[],
              metadata = nothing)
    Task(id, title, description, open, priority, tags, metadata, now())
end

# Pretty printing
function Base.show(io::IO, t::Task)
    icon = Dict(open => "[ ]", in_progress => "[~]",
                done => "[x]", cancelled => "[-]")
    prio = Dict(low => " ", medium => "!",
                high => "!!", critical => "!!!")
    tag_str = isempty(t.tags) ? "" : " [$(join(t.tags, ", "))]"
    print(io, "#$(t.id) $(icon[t.status]) $(prio[t.priority]) $(t.title)$(tag_str)")
end

# ============================================================
# Multiple dispatch
# ============================================================

"""Transition a task to a new status with validation."""
function transition(task::Task, new_status::Status)
    valid = Dict(
        open        => [in_progress, cancelled],
        in_progress => [open, done, cancelled],
        done        => [open],
        cancelled   => [open],
    )

    new_status ∈ valid[task.status] ||
        error("Cannot transition from $(task.status) to $new_status")

    Task(task.id, task.title, task.description, new_status,
         task.priority, task.tags, task.metadata, task.created_at)
end

# Dispatch on priority for scheduling weight
scheduling_weight(::Val{low}) = 1.0
scheduling_weight(::Val{medium}) = 2.0
scheduling_weight(::Val{high}) = 4.0
scheduling_weight(::Val{critical}) = 8.0
scheduling_weight(t::Task) = scheduling_weight(Val(t.priority))

# ============================================================
# Functional operations with Unicode
# ============================================================

# Custom operators
⊕(a::Task, b::Task) = scheduling_weight(a) + scheduling_weight(b)
≺(a::Task, b::Task) = Int(a.priority) < Int(b.priority)

"""Filter tasks by predicate and sort by priority."""
function filter_and_sort(tasks::Vector{<:Task}, predicate::Function)
    filtered = filter(predicate, tasks)
    sort(collect(filtered), by = t -> -Int(t.priority))
end

"""Group tasks by a key function."""
function group_by(f::Function, tasks)
    result = Dict{Any, Vector{eltype(tasks)}}()
    for task in tasks
        key = f(task)
        push!(get!(result, key, eltype(tasks)[]), task)
    end
    result
end

# ============================================================
# Statistics
# ============================================================

struct TaskStats
    total::Int
    by_status::Dict{Status, Int}
    by_priority::Dict{Priority, Int}
    completion_rate::Float64
    avg_tags::Float64
    priority_distribution::Vector{Float64}
end

function compute_stats(tasks::Vector{<:Task})
    n = length(tasks)
    n == 0 && return TaskStats(0, Dict(), Dict(), 0.0, 0.0, Float64[])

    by_status = Dict(s => count(t -> t.status == s, tasks) for s in instances(Status))
    by_priority = Dict(p => count(t -> t.priority == p, tasks) for p in instances(Priority))

    done_count = get(by_status, done, 0)
    total_tags = sum(t -> length(t.tags), tasks)

    # Priority distribution as probability vector
    prio_dist = [get(by_priority, p, 0) / n for p in instances(Priority)]

    TaskStats(n, by_status, by_priority,
              done_count / n * 100,
              total_tags / n,
              prio_dist)
end

function Base.show(io::IO, s::TaskStats)
    println(io, "=== Task Statistics ===")
    println(io, "Total: $(s.total)")
    @printf(io, "Completion: %.1f%%\n", s.completion_rate)
    @printf(io, "Avg tags/task: %.1f\n", s.avg_tags)
    println(io, "\nBy status:")
    for (status, count) in sort(collect(s.by_status), by=first)
        println(io, "  $status: $count")
    end
    println(io, "\nBy priority:")
    for (prio, count) in sort(collect(s.by_priority), by=first)
        println(io, "  $prio: $count")
    end
end

# ============================================================
# Simulation with linear algebra
# ============================================================

"""
    simulate_workload(n_tasks, n_workers, n_steps)

Monte Carlo simulation of task throughput using matrix operations.
Returns completion times as a vector.
"""
function simulate_workload(n_tasks::Int, n_workers::Int, n_steps::Int;
                           rng::AbstractRNG = MersenneTwister(42))
    # Task complexity matrix: each task has a cost vector
    costs = rand(rng, n_tasks, n_workers) .* 10.0

    # Worker efficiency (improves over time)
    efficiency = ones(n_workers)
    completion_times = zeros(n_tasks)

    for step in 1:n_steps
        # Update efficiency with learning curve
        efficiency .= 1.0 .+ 0.1 .* log.(step)

        # Assignment matrix (greedy)
        assignment = zeros(Bool, n_tasks, n_workers)
        available = trues(n_workers)

        for task in sortperm(vec(minimum(costs, dims=2)))
            completion_times[task] > 0 && continue

            worker = argmin([available[w] ? costs[task, w] / efficiency[w] : Inf
                           for w in 1:n_workers])

            if available[worker]
                assignment[task, worker] = true
                available[worker] = false

                # Check if task completes this step
                effective_cost = costs[task, worker] / efficiency[worker]
                if effective_cost ≤ step
                    completion_times[task] = step
                end
            end
        end
    end

    completion_times
end

"""Compute throughput statistics from simulation."""
function analyze_throughput(times::Vector{Float64})
    completed = filter(>(0), times)
    n = length(completed)

    (
        completed = n,
        total = length(times),
        mean_time = n > 0 ? mean(completed) : NaN,
        std_time = n > 0 ? std(completed) : NaN,
        median_time = n > 0 ? median(completed) : NaN,
        percentile_95 = n > 0 ? quantile(completed, 0.95) : NaN,
    )
end

# ============================================================
# Metaprogramming
# ============================================================

"""Generate accessor functions for Task fields."""
macro generate_accessors(fields...)
    exprs = Expr[]
    for field in fields
        fname = Symbol("get_", field)
        push!(exprs, quote
            $(esc(fname))(t::Task) = getfield(t, $(QuoteNode(field)))
        end)
    end
    Expr(:block, exprs...)
end

@generate_accessors title status priority tags

"""Benchmark a function with timing."""
macro timed(label, expr)
    quote
        local t0 = time_ns()
        local result = $(esc(expr))
        local elapsed = (time_ns() - t0) / 1e6
        @printf("%s: %.2f ms\n", $(esc(label)), elapsed)
        result
    end
end

# ============================================================
# Main
# ============================================================

function main()
    println("Task Analytics v1.0\n")

    # Create tasks
    tasks = Task{Nothing}[
        Task(1, "Implement syntax highlighting",
             priority=high, tags=["feature", "syntax"]),
        Task(2, "Fix cursor blinking",
             priority=low, tags=["bug"]),
        Task(3, "Add split view",
             priority=medium, tags=["feature", "ui"]),
        Task(4, "Write documentation",
             priority=medium, tags=["docs"]),
        Task(5, "Performance profiling",
             priority=high, tags=["perf"]),
    ]

    # Transition some tasks
    tasks[1] = transition(tasks[1], in_progress)
    tasks[2] = transition(tasks[2], done)

    # Display
    println("All tasks:")
    for task in sort(tasks, by=t -> -Int(t.priority))
        println("  ", task)
    end

    # Stats
    println()
    stats = compute_stats(tasks)
    println(stats)

    # Grouped
    println("\nBy status:")
    for (status, group) in group_by(t -> t.status, tasks)
        println("  $status: $(length(group)) tasks")
    end

    # Simulation
    println("\n=== Workload Simulation ===")
    times = @timed "Simulation" simulate_workload(50, 4, 100)
    throughput = analyze_throughput(times)
    @printf("Completed: %d/%d\n", throughput.completed, throughput.total)
    @printf("Mean time: %.1f steps\n", throughput.mean_time)
    @printf("95th percentile: %.1f steps\n", throughput.percentile_95)
end

end # module

# Run
using .TaskAnalytics
TaskAnalytics.main()

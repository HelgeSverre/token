<!--
  Svelte Syntax Highlighting Test
  A kanban board with drag-and-drop, stores, and transitions.
-->

<script lang="ts">
    import { onMount, createEventDispatcher, tick } from 'svelte';
    import { flip } from 'svelte/animate';
    import { fade, fly, crossfade } from 'svelte/transition';
    import { quintOut } from 'svelte/easing';
    import { writable, derived, type Writable } from 'svelte/store';

    // Types
    interface Task {
        id: string;
        title: string;
        description?: string;
        priority: 'low' | 'medium' | 'high' | 'critical';
        tags: string[];
        assignee?: string;
        createdAt: Date;
    }

    interface Column {
        id: string;
        title: string;
        tasks: Task[];
        color: string;
    }

    type DragState = {
        taskId: string;
        sourceColumn: string;
        overColumn: string | null;
    } | null;

    // Props
    export let projectName: string = 'My Project';
    export let initialColumns: Column[] = [];

    // Event dispatcher
    const dispatch = createEventDispatcher<{
        'task-moved': { taskId: string; from: string; to: string };
        'task-created': { task: Task; columnId: string };
        'task-deleted': { taskId: string; columnId: string };
    }>();

    // Stores
    const columns: Writable<Column[]> = writable(initialColumns);
    const searchQuery = writable('');
    const selectedTags = writable<Set<string>>(new Set());
    const dragState = writable<DragState>(null);

    // Derived stores
    const allTags = derived(columns, ($columns) => {
        const tags = new Set<string>();
        $columns.forEach((col) =>
            col.tasks.forEach((task) =>
                task.tags.forEach((tag) => tags.add(tag))
            )
        );
        return [...tags].sort();
    });

    const filteredColumns = derived(
        [columns, searchQuery, selectedTags],
        ([$columns, $query, $tags]) => {
            return $columns.map((col) => ({
                ...col,
                tasks: col.tasks.filter((task) => {
                    const matchesSearch =
                        !$query ||
                        task.title.toLowerCase().includes($query.toLowerCase()) ||
                        task.description?.toLowerCase().includes($query.toLowerCase());

                    const matchesTags =
                        $tags.size === 0 ||
                        task.tags.some((tag) => $tags.has(tag));

                    return matchesSearch && matchesTags;
                }),
            }));
        }
    );

    const taskCount = derived(columns, ($columns) =>
        $columns.reduce((sum, col) => sum + col.tasks.length, 0)
    );

    // Crossfade for smooth task movement
    const [send, receive] = crossfade({
        duration: 300,
        easing: quintOut,
        fallback: fade,
    });

    // Local state
    let newTaskTitle = '';
    let addingToColumn: string | null = null;
    let inputRef: HTMLInputElement;

    // Priority colors
    const priorityColors: Record<Task['priority'], string> = {
        low: '#22c55e',
        medium: '#f59e0b',
        high: '#ef4444',
        critical: '#dc2626',
    };

    // Drag and drop handlers
    function handleDragStart(taskId: string, columnId: string) {
        $dragState = { taskId, sourceColumn: columnId, overColumn: null };
    }

    function handleDragOver(columnId: string) {
        if ($dragState && $dragState.sourceColumn !== columnId) {
            $dragState = { ...$dragState, overColumn: columnId };
        }
    }

    function handleDrop(targetColumnId: string) {
        if (!$dragState) return;

        const { taskId, sourceColumn } = $dragState;
        if (sourceColumn === targetColumnId) {
            $dragState = null;
            return;
        }

        columns.update((cols) => {
            const sourcCol = cols.find((c) => c.id === sourceColumn);
            const targetCol = cols.find((c) => c.id === targetColumnId);
            if (!sourcCol || !targetCol) return cols;

            const taskIndex = sourcCol.tasks.findIndex((t) => t.id === taskId);
            if (taskIndex === -1) return cols;

            const [task] = sourcCol.tasks.splice(taskIndex, 1);
            targetCol.tasks.push(task);

            return [...cols];
        });

        dispatch('task-moved', {
            taskId,
            from: sourceColumn,
            to: targetColumnId,
        });

        $dragState = null;
    }

    function handleDragEnd() {
        $dragState = null;
    }

    // Task CRUD
    async function addTask(columnId: string) {
        if (!newTaskTitle.trim()) return;

        const task: Task = {
            id: `task-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            title: newTaskTitle.trim(),
            priority: 'medium',
            tags: [],
            createdAt: new Date(),
        };

        columns.update((cols) =>
            cols.map((col) =>
                col.id === columnId
                    ? { ...col, tasks: [...col.tasks, task] }
                    : col
            )
        );

        dispatch('task-created', { task, columnId });
        newTaskTitle = '';
        addingToColumn = null;
    }

    function deleteTask(taskId: string, columnId: string) {
        columns.update((cols) =>
            cols.map((col) =>
                col.id === columnId
                    ? { ...col, tasks: col.tasks.filter((t) => t.id !== taskId) }
                    : col
            )
        );
        dispatch('task-deleted', { taskId, columnId });
    }

    function toggleTag(tag: string) {
        selectedTags.update((tags) => {
            const next = new Set(tags);
            if (next.has(tag)) next.delete(tag);
            else next.add(tag);
            return next;
        });
    }

    async function startAdding(columnId: string) {
        addingToColumn = columnId;
        await tick();
        inputRef?.focus();
    }

    onMount(() => {
        if (initialColumns.length === 0) {
            columns.set([
                { id: 'todo', title: 'To Do', tasks: [], color: '#6366f1' },
                { id: 'progress', title: 'In Progress', tasks: [], color: '#f59e0b' },
                { id: 'review', title: 'Review', tasks: [], color: '#3b82f6' },
                { id: 'done', title: 'Done', tasks: [], color: '#22c55e' },
            ]);
        }
    });

    // Reactive statement
    $: completionRate =
        $taskCount > 0
            ? Math.round(
                  (($columns.find((c) => c.id === 'done')?.tasks.length ?? 0) /
                      $taskCount) *
                      100
              )
            : 0;
</script>

<div class="kanban">
    <header class="kanban-header">
        <div class="header-left">
            <h1>{projectName}</h1>
            <span class="task-count">{$taskCount} tasks</span>
            {#if completionRate > 0}
                <span class="completion" transition:fade>
                    {completionRate}% complete
                </span>
            {/if}
        </div>

        <div class="header-right">
            <input
                type="search"
                placeholder="Search tasks..."
                bind:value={$searchQuery}
                class="search-input"
            />
        </div>
    </header>

    <!-- Tag filter bar -->
    {#if $allTags.length > 0}
        <div class="tag-bar" transition:fly={{ y: -10 }}>
            {#each $allTags as tag (tag)}
                <button
                    class="tag-filter"
                    class:active={$selectedTags.has(tag)}
                    on:click={() => toggleTag(tag)}
                >
                    {tag}
                </button>
            {/each}
            {#if $selectedTags.size > 0}
                <button class="clear-filters" on:click={() => selectedTags.set(new Set())}>
                    Clear
                </button>
            {/if}
        </div>
    {/if}

    <!-- Columns -->
    <div class="columns">
        {#each $filteredColumns as column (column.id)}
            <div
                class="column"
                class:drag-over={$dragState?.overColumn === column.id}
                on:dragover|preventDefault={() => handleDragOver(column.id)}
                on:drop|preventDefault={() => handleDrop(column.id)}
                on:dragleave={() => {
                    if ($dragState) $dragState = { ...$dragState, overColumn: null };
                }}
            >
                <div class="column-header">
                    <div
                        class="column-indicator"
                        style="background-color: {column.color}"
                    />
                    <h2>{column.title}</h2>
                    <span class="column-count">{column.tasks.length}</span>
                </div>

                <div class="task-list">
                    {#each column.tasks as task (task.id)}
                        <div
                            class="task-card"
                            draggable="true"
                            on:dragstart={() => handleDragStart(task.id, column.id)}
                            on:dragend={handleDragEnd}
                            in:receive={{ key: task.id }}
                            out:send={{ key: task.id }}
                            animate:flip={{ duration: 300 }}
                        >
                            <div class="task-header">
                                <span
                                    class="priority-dot"
                                    style="background: {priorityColors[task.priority]}"
                                    title={task.priority}
                                />
                                <span class="task-title">{task.title}</span>
                                <button
                                    class="delete-btn"
                                    on:click|stopPropagation={() =>
                                        deleteTask(task.id, column.id)}
                                    aria-label="Delete task"
                                >
                                    &times;
                                </button>
                            </div>

                            {#if task.description}
                                <p class="task-description">{task.description}</p>
                            {/if}

                            {#if task.tags.length > 0}
                                <div class="task-tags">
                                    {#each task.tags as tag}
                                        <span class="tag">{tag}</span>
                                    {/each}
                                </div>
                            {/if}

                            {#if task.assignee}
                                <div class="task-assignee">
                                    <span class="avatar">{task.assignee[0]}</span>
                                    {task.assignee}
                                </div>
                            {/if}
                        </div>
                    {:else}
                        <p class="empty-column" in:fade>No tasks</p>
                    {/each}
                </div>

                <!-- Add task form -->
                {#if addingToColumn === column.id}
                    <form
                        class="add-task-form"
                        on:submit|preventDefault={() => addTask(column.id)}
                        transition:fly={{ y: 10, duration: 200 }}
                    >
                        <input
                            bind:this={inputRef}
                            bind:value={newTaskTitle}
                            placeholder="Task title..."
                            on:keydown={(e) => e.key === 'Escape' && (addingToColumn = null)}
                        />
                        <div class="form-actions">
                            <button type="submit" class="btn-add">Add</button>
                            <button
                                type="button"
                                class="btn-cancel"
                                on:click={() => (addingToColumn = null)}
                            >
                                Cancel
                            </button>
                        </div>
                    </form>
                {:else}
                    <button
                        class="add-task-btn"
                        on:click={() => startAdding(column.id)}
                    >
                        + Add task
                    </button>
                {/if}
            </div>
        {/each}
    </div>
</div>

<style>
    .kanban {
        display: flex;
        flex-direction: column;
        height: 100vh;
        background: #f1f5f9;
        font-family: system-ui, -apple-system, sans-serif;
    }

    .kanban-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 1rem 1.5rem;
        background: white;
        border-bottom: 1px solid #e2e8f0;
    }

    .header-left {
        display: flex;
        align-items: center;
        gap: 1rem;
    }

    .header-left h1 {
        font-size: 1.25rem;
        margin: 0;
    }

    .task-count, .completion {
        font-size: 0.8125rem;
        color: #64748b;
        padding: 0.25rem 0.75rem;
        background: #f1f5f9;
        border-radius: 999px;
    }

    .columns {
        display: flex;
        gap: 1rem;
        padding: 1.5rem;
        overflow-x: auto;
        flex: 1;
    }

    .column {
        flex: 0 0 300px;
        display: flex;
        flex-direction: column;
        background: white;
        border-radius: 12px;
        padding: 1rem;
        max-height: calc(100vh - 160px);
        transition: box-shadow 0.2s;
    }

    .column.drag-over {
        box-shadow: 0 0 0 2px #3b82f6;
    }

    .column-header {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        margin-bottom: 1rem;
    }

    .column-indicator {
        width: 8px;
        height: 8px;
        border-radius: 50%;
    }

    .column-header h2 {
        font-size: 0.875rem;
        font-weight: 600;
        margin: 0;
        flex: 1;
    }

    .column-count {
        font-size: 0.75rem;
        color: #94a3b8;
        background: #f1f5f9;
        padding: 0.125rem 0.5rem;
        border-radius: 999px;
    }

    .task-list {
        flex: 1;
        overflow-y: auto;
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }

    .task-card {
        padding: 0.75rem;
        border: 1px solid #e2e8f0;
        border-radius: 8px;
        cursor: grab;
        transition: box-shadow 0.15s;
    }

    .task-card:hover {
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
    }

    .task-card:active {
        cursor: grabbing;
    }

    .task-header {
        display: flex;
        align-items: flex-start;
        gap: 0.5rem;
    }

    .priority-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        margin-top: 0.375rem;
        flex-shrink: 0;
    }

    .task-title {
        flex: 1;
        font-size: 0.875rem;
        font-weight: 500;
    }

    .delete-btn {
        opacity: 0;
        background: none;
        border: none;
        font-size: 1.25rem;
        cursor: pointer;
        color: #94a3b8;
        padding: 0;
        line-height: 1;
    }

    .task-card:hover .delete-btn {
        opacity: 1;
    }

    .task-tags {
        display: flex;
        gap: 0.25rem;
        margin-top: 0.5rem;
        flex-wrap: wrap;
    }

    .tag {
        font-size: 0.6875rem;
        padding: 0.125rem 0.5rem;
        border-radius: 999px;
        background: #ede9fe;
        color: #6d28d9;
    }

    .add-task-btn {
        width: 100%;
        padding: 0.5rem;
        border: 1px dashed #cbd5e1;
        border-radius: 8px;
        background: none;
        color: #64748b;
        cursor: pointer;
        font-size: 0.8125rem;
        margin-top: 0.5rem;
    }

    .add-task-btn:hover {
        background: #f8fafc;
        border-color: #94a3b8;
    }
</style>

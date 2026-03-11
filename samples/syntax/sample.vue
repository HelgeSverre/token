<template>
    <div class="task-manager" :class="{ 'dark-mode': isDark }">
        <header class="header">
            <h1>{{ title }}</h1>
            <span class="task-count">{{ completedCount }} / {{ tasks.length }}</span>
            <button @click="toggleTheme" class="theme-toggle">
                {{ isDark ? '☀️' : '🌙' }}
            </button>
        </header>

        <form @submit.prevent="addTask" class="add-form">
            <input
                v-model.trim="newTaskText"
                type="text"
                placeholder="Add a new task..."
                :disabled="isLoading"
                ref="taskInput"
            />
            <select v-model="newTaskPriority">
                <option value="low">Low</option>
                <option value="medium">Medium</option>
                <option value="high">High</option>
            </select>
            <button type="submit" :disabled="!newTaskText">Add</button>
        </form>

        <div v-if="isLoading" class="loading">
            <span class="spinner"></span>
            Loading tasks...
        </div>

        <template v-else>
            <div class="filters">
                <button
                    v-for="f in filters"
                    :key="f.value"
                    :class="['filter-btn', { active: filter === f.value }]"
                    @click="filter = f.value"
                >
                    {{ f.label }}
                </button>
            </div>

            <TransitionGroup name="list" tag="ul" class="task-list">
                <li
                    v-for="task in filteredTasks"
                    :key="task.id"
                    :class="['task-item', `priority-${task.priority}`, { done: task.done }]"
                >
                    <input
                        type="checkbox"
                        :checked="task.done"
                        @change="toggleTask(task.id)"
                        :id="`task-${task.id}`"
                    />
                    <label :for="`task-${task.id}`">{{ task.text }}</label>
                    <span class="priority-badge">{{ task.priority }}</span>
                    <button
                        @click.stop="removeTask(task.id)"
                        class="remove-btn"
                        :aria-label="`Remove ${task.text}`"
                    >
                        &times;
                    </button>
                </li>
            </TransitionGroup>

            <p v-if="filteredTasks.length === 0" class="empty-state">
                No {{ filter !== 'all' ? filter : '' }} tasks found.
            </p>
        </template>

        <footer class="footer">
            <slot name="footer">
                <p>Built with Vue 3 &amp; Composition API</p>
            </slot>
        </footer>
    </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, nextTick } from 'vue';

interface Task {
    id: number;
    text: string;
    done: boolean;
    priority: 'low' | 'medium' | 'high';
    createdAt: Date;
}

type FilterValue = 'all' | 'active' | 'done';

interface FilterOption {
    label: string;
    value: FilterValue;
}

// Props and emits
const props = withDefaults(defineProps<{
    title?: string;
    initialTasks?: Task[];
}>(), {
    title: 'Task Manager',
    initialTasks: () => [],
});

const emit = defineEmits<{
    (e: 'task-added', task: Task): void;
    (e: 'task-removed', id: number): void;
    (e: 'task-toggled', id: number, done: boolean): void;
}>();

// Reactive state
const tasks = ref<Task[]>([...props.initialTasks]);
const newTaskText = ref('');
const newTaskPriority = ref<Task['priority']>('medium');
const filter = ref<FilterValue>('all');
const isLoading = ref(false);
const isDark = ref(false);
const taskInput = ref<HTMLInputElement | null>(null);
let nextId = 1;

const filters: FilterOption[] = [
    { label: 'All', value: 'all' },
    { label: 'Active', value: 'active' },
    { label: 'Done', value: 'done' },
];

// Computed properties
const filteredTasks = computed(() => {
    switch (filter.value) {
        case 'active':
            return tasks.value.filter(t => !t.done);
        case 'done':
            return tasks.value.filter(t => t.done);
        default:
            return tasks.value;
    }
});

const completedCount = computed(() =>
    tasks.value.filter(t => t.done).length
);

// Methods
function addTask() {
    if (!newTaskText.value) return;

    const task: Task = {
        id: nextId++,
        text: newTaskText.value,
        done: false,
        priority: newTaskPriority.value,
        createdAt: new Date(),
    };

    tasks.value.push(task);
    emit('task-added', task);
    newTaskText.value = '';

    nextTick(() => {
        taskInput.value?.focus();
    });
}

function removeTask(id: number) {
    tasks.value = tasks.value.filter(t => t.id !== id);
    emit('task-removed', id);
}

function toggleTask(id: number) {
    const task = tasks.value.find(t => t.id === id);
    if (task) {
        task.done = !task.done;
        emit('task-toggled', id, task.done);
    }
}

function toggleTheme() {
    isDark.value = !isDark.value;
}

// Watchers
watch(
    () => tasks.value.length,
    (newLen, oldLen) => {
        console.log(`Tasks changed: ${oldLen} → ${newLen}`);
    }
);

// Lifecycle
onMounted(async () => {
    isLoading.value = true;
    try {
        // Simulate API fetch
        await new Promise(resolve => setTimeout(resolve, 500));
        if (tasks.value.length === 0) {
            tasks.value = [
                { id: nextId++, text: 'Learn Vue 3', done: true, priority: 'high', createdAt: new Date() },
                { id: nextId++, text: 'Build a component', done: false, priority: 'medium', createdAt: new Date() },
                { id: nextId++, text: 'Write tests', done: false, priority: 'low', createdAt: new Date() },
            ];
        }
    } finally {
        isLoading.value = false;
    }
});
</script>

<style scoped>
.task-manager {
    max-width: 600px;
    margin: 0 auto;
    padding: 2rem;
    font-family: system-ui, -apple-system, sans-serif;
}

.task-manager.dark-mode {
    background: #1a1a2e;
    color: #e0e0e0;
}

.header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1.5rem;
}

.header h1 {
    margin: 0;
    font-size: 1.5rem;
    flex: 1;
}

.task-count {
    font-size: 0.875rem;
    opacity: 0.7;
}

.add-form {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
}

.add-form input {
    flex: 1;
    padding: 0.5rem 0.75rem;
    border: 1px solid #ccc;
    border-radius: 4px;
}

.task-list {
    list-style: none;
    padding: 0;
}

.task-item {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem;
    border-radius: 6px;
    transition: all 0.2s ease;
}

.task-item.done label {
    text-decoration: line-through;
    opacity: 0.6;
}

.task-item.priority-high {
    border-left: 3px solid #e74c3c;
}

.task-item.priority-medium {
    border-left: 3px solid #f39c12;
}

.task-item.priority-low {
    border-left: 3px solid #27ae60;
}

.priority-badge {
    font-size: 0.75rem;
    padding: 0.125rem 0.5rem;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.08);
    text-transform: uppercase;
}

.remove-btn {
    margin-left: auto;
    background: none;
    border: none;
    font-size: 1.25rem;
    cursor: pointer;
    opacity: 0.4;
    transition: opacity 0.15s;
}

.remove-btn:hover {
    opacity: 1;
    color: #e74c3c;
}

/* Transition group animations */
.list-enter-active,
.list-leave-active {
    transition: all 0.3s ease;
}

.list-enter-from,
.list-leave-to {
    opacity: 0;
    transform: translateX(-20px);
}

.empty-state {
    text-align: center;
    opacity: 0.5;
    padding: 2rem;
}
</style>

/// Dart Syntax Highlighting Test
/// A Flutter-style widget tree with state management and async patterns.

import 'dart:async';
import 'dart:collection';
import 'dart:convert';
import 'dart:math' show Random, pi;

// Constants
const double kDefaultPadding = 16.0;
const int kMaxRetries = 3;
const Duration kAnimationDuration = Duration(milliseconds: 300);

// Enum with enhanced features
enum Priority implements Comparable<Priority> {
  low(0, 'Low'),
  medium(1, 'Medium'),
  high(2, 'High'),
  critical(3, 'Critical');

  const Priority(this.value, this.label);
  final int value;
  final String label;

  @override
  int compareTo(Priority other) => value.compareTo(other.value);

  bool operator >(Priority other) => value > other.value;
}

// Sealed class hierarchy (Dart 3)
sealed class Result<T> {
  const Result();
}

class Success<T> extends Result<T> {
  final T value;
  const Success(this.value);
}

class Failure<T> extends Result<T> {
  final String message;
  final Object? error;
  const Failure(this.message, [this.error]);
}

class Loading<T> extends Result<T> {
  const Loading();
}

// Extension type (Dart 3.3)
extension type UserId(String value) {
  UserId.generate() : value = 'usr_${DateTime.now().millisecondsSinceEpoch}';

  bool get isValid => value.startsWith('usr_') && value.length > 4;
}

// Mixin
mixin Timestamped {
  DateTime _createdAt = DateTime.now();
  DateTime? _updatedAt;

  DateTime get createdAt => _createdAt;
  DateTime? get updatedAt => _updatedAt;

  void touch() => _updatedAt = DateTime.now();

  Duration get age => DateTime.now().difference(_createdAt);
}

// Abstract class with factory
abstract class Serializable<T> {
  Map<String, dynamic> toJson();
  String serialize() => jsonEncode(toJson());
}

// Data class with named parameters, mixins
class Task with Timestamped implements Serializable<Task> {
  final String id;
  final String title;
  final String? description;
  final Priority priority;
  final bool completed;
  final List<String> tags;

  Task({
    required this.id,
    required this.title,
    this.description,
    this.priority = Priority.medium,
    this.completed = false,
    this.tags = const [],
  });

  // Copyable pattern
  Task copyWith({
    String? title,
    String? description,
    Priority? priority,
    bool? completed,
    List<String>? tags,
  }) {
    return Task(
      id: id,
      title: title ?? this.title,
      description: description ?? this.description,
      priority: priority ?? this.priority,
      completed: completed ?? this.completed,
      tags: tags ?? this.tags,
    );
  }

  @override
  Map<String, dynamic> toJson() => {
    'id': id,
    'title': title,
    'description': description,
    'priority': priority.name,
    'completed': completed,
    'tags': tags,
    'createdAt': createdAt.toIso8601String(),
  };

  factory Task.fromJson(Map<String, dynamic> json) => Task(
    id: json['id'] as String,
    title: json['title'] as String,
    description: json['description'] as String?,
    priority: Priority.values.byName(json['priority'] as String),
    completed: json['completed'] as bool? ?? false,
    tags: (json['tags'] as List?)?.cast<String>() ?? [],
  );

  @override
  String toString() => 'Task($id: $title [${priority.label}])';
}

// Generic repository with streams
class Repository<T extends Serializable<T>> {
  final Map<String, T> _store = {};
  final _controller = StreamController<List<T>>.broadcast();

  Stream<List<T>> get stream => _controller.stream;
  List<T> get items => UnmodifiableListView(_store.values.toList());

  void add(String id, T item) {
    _store[id] = item;
    _notify();
  }

  T? get(String id) => _store[id];

  bool remove(String id) {
    final removed = _store.remove(id) != null;
    if (removed) _notify();
    return removed;
  }

  void update(String id, T Function(T) updater) {
    final current = _store[id];
    if (current != null) {
      _store[id] = updater(current);
      _notify();
    }
  }

  void _notify() => _controller.add(items);

  Future<void> dispose() => _controller.close();
}

// State management with ChangeNotifier pattern
class TaskStore {
  final _repo = Repository<Task>();
  Result<List<Task>> _state = const Loading();
  String _filter = '';

  Result<List<Task>> get state => _state;

  List<Task> get filteredTasks {
    if (_state case Success(value: final tasks)) {
      return tasks.where((t) {
        if (_filter.isEmpty) return true;
        return t.title.toLowerCase().contains(_filter.toLowerCase()) ||
               t.tags.any((tag) => tag.contains(_filter));
      }).toList()
        ..sort((a, b) => b.priority.compareTo(a.priority));
    }
    return [];
  }

  void setFilter(String filter) => _filter = filter;

  Future<void> loadTasks() async {
    _state = const Loading();

    try {
      // Simulate API call
      await Future.delayed(const Duration(seconds: 1));

      final tasks = List.generate(10, (i) => Task(
        id: 'task_$i',
        title: 'Task ${i + 1}',
        description: i.isEven ? 'Description for task ${i + 1}' : null,
        priority: Priority.values[i % Priority.values.length],
        tags: ['sprint-${i ~/ 3}', if (i.isEven) 'frontend' else 'backend'],
      ));

      for (final task in tasks) {
        _repo.add(task.id, task);
      }

      _state = Success(tasks);
    } catch (e) {
      _state = Failure('Failed to load tasks', e);
    }
  }

  void toggleTask(String id) {
    _repo.update(id, (task) => task.copyWith(completed: !task.completed));
  }
}

// Extension methods
extension StringX on String {
  String truncate(int maxLength, {String ellipsis = '...'}) {
    if (length <= maxLength) return this;
    return '${substring(0, maxLength - ellipsis.length)}$ellipsis';
  }

  String get initials => split(' ')
      .where((w) => w.isNotEmpty)
      .take(2)
      .map((w) => w[0].toUpperCase())
      .join();
}

extension ListX<T> on List<T> {
  List<T> sortedBy<K extends Comparable>(K Function(T) keyOf) =>
      [...this]..sort((a, b) => keyOf(a).compareTo(keyOf(b)));

  Map<K, List<T>> groupBy<K>(K Function(T) keyOf) {
    final map = <K, List<T>>{};
    for (final item in this) {
      (map[keyOf(item)] ??= []).add(item);
    }
    return map;
  }
}

// Async generator
Stream<int> fibonacci() async* {
  int a = 0, b = 1;
  while (true) {
    yield a;
    (a, b) = (b, a + b);  // Record destructuring
    await Future.delayed(const Duration(milliseconds: 100));
  }
}

// Pattern matching (Dart 3)
String describeResult(Result<dynamic> result) => switch (result) {
  Success(value: final v) when v is List && v.isEmpty => 'Empty collection',
  Success(value: final v) => 'Success: $v',
  Failure(message: final m, error: final e?) => 'Error: $m ($e)',
  Failure(message: final m) => 'Error: $m',
  Loading() => 'Loading...',
};

// Entry point
void main() async {
  final store = TaskStore();
  await store.loadTasks();

  // Pattern match on state
  switch (store.state) {
    case Success(value: final tasks):
      print('Loaded ${tasks.length} tasks');
      for (final task in store.filteredTasks) {
        final status = task.completed ? '✓' : '○';
        print('  $status ${task.title} [${task.priority.label}]');
      }
    case Failure(message: final msg):
      print('Error: $msg');
    case Loading():
      print('Still loading...');
  }

  // Use extensions
  print('Hello World'.initials);  // HW
  print('A very long task title here'.truncate(15));

  // Async stream
  await for (final n in fibonacci().take(10)) {
    print('fib: $n');
  }
}

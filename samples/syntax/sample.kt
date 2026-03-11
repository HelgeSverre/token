/**
 * Kotlin Syntax Highlighting Test
 * A reactive event processing pipeline with coroutines.
 */

package com.example.events

import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import java.time.Instant
import java.time.Duration
import java.util.concurrent.ConcurrentHashMap
import kotlin.math.roundToInt

// Sealed class hierarchy for events
sealed class Event {
    abstract val id: String
    abstract val timestamp: Instant

    data class UserAction(
        override val id: String,
        override val timestamp: Instant,
        val userId: String,
        val action: ActionType,
        val metadata: Map<String, Any?> = emptyMap()
    ) : Event()

    data class SystemAlert(
        override val id: String,
        override val timestamp: Instant,
        val severity: Severity,
        val message: String,
        val source: String
    ) : Event()

    data class Metric(
        override val id: String,
        override val timestamp: Instant,
        val name: String,
        val value: Double,
        val tags: Map<String, String> = emptyMap()
    ) : Event()
}

enum class ActionType { LOGIN, LOGOUT, PURCHASE, VIEW, SEARCH }
enum class Severity { INFO, WARNING, ERROR, CRITICAL }

// Value class (inline class) for type safety
@JvmInline
value class UserId(val value: String) {
    init {
        require(value.isNotBlank()) { "UserId cannot be blank" }
    }
}

@JvmInline
value class EventCount(val value: Int) {
    operator fun plus(other: EventCount) = EventCount(value + other.value)
}

// Data class with default values and copy
data class ProcessingConfig(
    val batchSize: Int = 100,
    val windowDuration: Duration = Duration.ofSeconds(30),
    val maxRetries: Int = 3,
    val parallelism: Int = Runtime.getRuntime().availableProcessors(),
    val filters: List<(Event) -> Boolean> = emptyList()
) {
    fun withFilter(predicate: (Event) -> Boolean) = copy(
        filters = filters + predicate
    )
}

// Interface with default methods
interface EventSink {
    suspend fun emit(event: Event)
    suspend fun emitAll(events: List<Event>) {
        events.forEach { emit(it) }
    }
    suspend fun flush() {}
}

// Companion object and factory pattern
class EventProcessor private constructor(
    private val config: ProcessingConfig,
    private val sinks: List<EventSink>
) {
    companion object {
        fun create(block: Builder.() -> Unit): EventProcessor {
            val builder = Builder()
            builder.block()
            return builder.build()
        }
    }

    class Builder {
        var config = ProcessingConfig()
        private val sinks = mutableListOf<EventSink>()

        fun addSink(sink: EventSink) = apply { sinks.add(sink) }
        fun configure(block: ProcessingConfig.() -> ProcessingConfig) = apply {
            config = config.block()
        }

        fun build(): EventProcessor {
            require(sinks.isNotEmpty()) { "At least one sink required" }
            return EventProcessor(config, sinks.toList())
        }
    }

    // Process events using Flow
    fun process(events: Flow<Event>): Flow<ProcessedBatch> = events
        .filter { event -> config.filters.all { it(event) } }
        .chunked(config.batchSize)
        .map { batch -> processBatch(batch) }
        .flowOn(Dispatchers.Default)
        .catch { e -> emit(ProcessedBatch.failed(e)) }

    private suspend fun processBatch(events: List<Event>): ProcessedBatch {
        val grouped = events.groupBy { it::class }
        val stats = buildMap {
            put("total", events.size)
            put("userActions", grouped[Event.UserAction::class]?.size ?: 0)
            put("alerts", grouped[Event.SystemAlert::class]?.size ?: 0)
            put("metrics", grouped[Event.Metric::class]?.size ?: 0)
        }

        // Fan out to sinks
        coroutineScope {
            sinks.map { sink ->
                async { sink.emitAll(events) }
            }.awaitAll()
        }

        return ProcessedBatch(events.size, stats, Instant.now())
    }
}

data class ProcessedBatch(
    val count: Int,
    val stats: Map<String, Int>,
    val processedAt: Instant
) {
    companion object {
        fun failed(error: Throwable) = ProcessedBatch(
            0, mapOf("error" to 1), Instant.now()
        )
    }
}

// Extension functions
fun <T> Flow<T>.chunked(size: Int): Flow<List<T>> = flow {
    val buffer = mutableListOf<T>()
    collect { value ->
        buffer.add(value)
        if (buffer.size >= size) {
            emit(buffer.toList())
            buffer.clear()
        }
    }
    if (buffer.isNotEmpty()) emit(buffer.toList())
}

fun List<Event>.summarize(): String = buildString {
    appendLine("Events: ${this@summarize.size}")
    groupBy { it::class.simpleName }.forEach { (type, events) ->
        appendLine("  $type: ${events.size}")
    }
}

// Inline function with reified type
inline fun <reified T : Event> List<Event>.filterByType(): List<T> =
    filterIsInstance<T>()

// Object declaration (singleton)
object EventIdGenerator {
    private var counter = 0L

    @Synchronized
    fun next(): String = "evt_${++counter}_${System.nanoTime()}"
}

// DSL builder
class AlertRuleBuilder {
    var name: String = ""
    var severity: Severity = Severity.WARNING
    private var condition: (Event) -> Boolean = { false }
    private var actions: MutableList<suspend (Event) -> Unit> = mutableListOf()

    fun `when`(predicate: (Event) -> Boolean) {
        condition = predicate
    }

    fun then(action: suspend (Event) -> Unit) {
        actions.add(action)
    }

    fun build() = AlertRule(name, severity, condition, actions.toList())
}

data class AlertRule(
    val name: String,
    val severity: Severity,
    val condition: (Event) -> Boolean,
    val actions: List<suspend (Event) -> Unit>
)

fun alertRule(block: AlertRuleBuilder.() -> Unit): AlertRule =
    AlertRuleBuilder().apply(block).build()

// Destructuring and when expressions
fun describeEvent(event: Event): String = when (event) {
    is Event.UserAction -> {
        val (_, _, userId, action, metadata) = event
        "User $userId performed $action${metadata.entries.joinToString { " ${it.key}=${it.value}" }}"
    }
    is Event.SystemAlert -> {
        val (_, _, severity, message, source) = event
        "[$severity] $source: $message"
    }
    is Event.Metric -> {
        val (_, _, name, value, tags) = event
        "$name = ${value.roundToInt()}${if (tags.isNotEmpty()) " (${tags.entries.joinToString()})" else ""}"
    }
}

// Coroutine-based main
suspend fun main() {
    val processor = EventProcessor.create {
        config = ProcessingConfig(batchSize = 50, parallelism = 4)
        addSink(object : EventSink {
            override suspend fun emit(event: Event) {
                println(describeEvent(event))
            }
        })
        configure { withFilter { it.timestamp.isAfter(Instant.now().minus(Duration.ofHours(1))) } }
    }

    val rule = alertRule {
        name = "High error rate"
        severity = Severity.CRITICAL
        `when` { it is Event.SystemAlert && it.severity == Severity.ERROR }
        then { event -> println("ALERT: ${describeEvent(event)}") }
    }

    val events = flow {
        repeat(200) { i ->
            emit(Event.UserAction(
                id = EventIdGenerator.next(),
                timestamp = Instant.now(),
                userId = "user_${i % 10}",
                action = ActionType.entries.random(),
                metadata = mapOf("page" to "/products/$i")
            ))
            delay(10)
        }
    }

    processor.process(events).collect { batch ->
        println("Processed batch: ${batch.count} events, stats=${batch.stats}")
    }
}

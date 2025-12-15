/**
 * Java Syntax Highlighting Test
 * 
 * This file demonstrates various Java syntax constructs.
 */

package com.example.syntax;

import java.util.*;
import java.util.concurrent.*;
import java.util.function.*;
import java.util.stream.*;
import java.io.*;
import java.nio.file.*;
import java.time.*;
import java.lang.annotation.*;

// Annotations
@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.TYPE, ElementType.METHOD})
@interface Logged {
    String value() default "";
}

@FunctionalInterface
interface Transformer<T, R> {
    R transform(T input);
    
    default Transformer<T, R> andLog() {
        return input -> {
            R result = transform(input);
            System.out.println("Result: " + result);
            return result;
        };
    }
}

// Sealed classes (Java 17+)
sealed interface Shape permits Circle, Rectangle, Triangle {
    double area();
    double perimeter();
}

// Enum with methods
enum Status {
    PENDING("Waiting"),
    ACTIVE("In Progress"),
    COMPLETED("Done"),
    FAILED("Error");
    
    private final String label;
    
    Status(String label) {
        this.label = label;
    }
    
    public String getLabel() {
        return label;
    }
}

// Record (Java 16+)
record Point(int x, int y) {
    // Compact constructor
    public Point {
        if (x < 0 || y < 0) {
            throw new IllegalArgumentException("Coordinates must be non-negative");
        }
    }
    
    // Static factory method
    public static Point origin() {
        return new Point(0, 0);
    }
    
    // Instance method
    public double distanceTo(Point other) {
        int dx = this.x - other.x;
        int dy = this.y - other.y;
        return Math.sqrt(dx * dx + dy * dy);
    }
}

// Generic record
record Pair<A, B>(A first, B second) {}

// Regular class implementing sealed interface
final class Circle implements Shape {
    private final double radius;
    
    public Circle(double radius) {
        this.radius = radius;
    }
    
    @Override
    public double area() {
        return Math.PI * radius * radius;
    }
    
    @Override
    public double perimeter() {
        return 2 * Math.PI * radius;
    }
    
    public double getRadius() {
        return radius;
    }
}

final class Rectangle implements Shape {
    private final double width;
    private final double height;
    
    public Rectangle(double width, double height) {
        this.width = width;
        this.height = height;
    }
    
    @Override
    public double area() {
        return width * height;
    }
    
    @Override
    public double perimeter() {
        return 2 * (width + height);
    }
}

final class Triangle implements Shape {
    private final double a, b, c;
    
    public Triangle(double a, double b, double c) {
        this.a = a;
        this.b = b;
        this.c = c;
    }
    
    @Override
    public double area() {
        double s = (a + b + c) / 2;
        return Math.sqrt(s * (s - a) * (s - b) * (s - c));
    }
    
    @Override
    public double perimeter() {
        return a + b + c;
    }
}

// Abstract class
abstract class Entity {
    protected final UUID id;
    protected LocalDateTime createdAt;
    
    protected Entity() {
        this.id = UUID.randomUUID();
        this.createdAt = LocalDateTime.now();
    }
    
    public UUID getId() {
        return id;
    }
    
    public abstract void validate();
}

// Generic class
class Container<T> {
    private final List<T> items = new ArrayList<>();
    
    public void add(T item) {
        items.add(item);
    }
    
    @SafeVarargs
    public final void addAll(T... items) {
        Collections.addAll(this.items, items);
    }
    
    public Optional<T> get(int index) {
        if (index < 0 || index >= items.size()) {
            return Optional.empty();
        }
        return Optional.of(items.get(index));
    }
    
    public <R> Container<R> map(Function<T, R> mapper) {
        Container<R> result = new Container<>();
        for (T item : items) {
            result.add(mapper.apply(item));
        }
        return result;
    }
    
    public Container<T> filter(Predicate<T> predicate) {
        Container<T> result = new Container<>();
        for (T item : items) {
            if (predicate.test(item)) {
                result.add(item);
            }
        }
        return result;
    }
    
    public int size() {
        return items.size();
    }
    
    public List<T> toList() {
        return Collections.unmodifiableList(items);
    }
}

// Main class
@Logged("Application entry point")
public class Sample {
    // Constants
    private static final String VERSION = "1.0.0";
    private static final double PI = 3.14159265359;
    private static final int MAX_SIZE = 1024;
    
    // Instance variables
    private final String name;
    private volatile boolean running;
    private transient int tempValue;
    
    // Static initializer
    static {
        System.out.println("Class loaded");
    }
    
    // Instance initializer
    {
        running = true;
        tempValue = 0;
    }
    
    // Constructor
    public Sample(String name) {
        this.name = Objects.requireNonNull(name, "Name cannot be null");
    }
    
    // Getters
    public String getName() {
        return name;
    }
    
    public boolean isRunning() {
        return running;
    }
    
    // Static method
    public static void printInfo() {
        System.out.println("Version: " + VERSION);
    }
    
    // Pattern matching for instanceof (Java 16+)
    public static String describe(Object obj) {
        if (obj instanceof String s && s.length() > 0) {
            return "Non-empty string: " + s;
        } else if (obj instanceof Integer i) {
            return "Integer: " + i;
        } else if (obj instanceof List<?> list) {
            return "List with " + list.size() + " elements";
        }
        return "Unknown: " + obj;
    }
    
    // Switch expression (Java 14+)
    public static String dayType(DayOfWeek day) {
        return switch (day) {
            case MONDAY, TUESDAY, WEDNESDAY, THURSDAY, FRIDAY -> "Weekday";
            case SATURDAY, SUNDAY -> "Weekend";
        };
    }
    
    // Pattern matching in switch (Java 21+)
    public static double calculateArea(Shape shape) {
        return switch (shape) {
            case Circle c -> Math.PI * c.getRadius() * c.getRadius();
            case Rectangle r -> r.area();
            case Triangle t -> t.area();
        };
    }
    
    // Lambda expressions
    public static void lambdaExamples() {
        // Simple lambdas
        Runnable runnable = () -> System.out.println("Running!");
        Consumer<String> printer = s -> System.out.println(s);
        Function<Integer, Integer> square = x -> x * x;
        BiFunction<Integer, Integer, Integer> add = (a, b) -> a + b;
        Predicate<Integer> isEven = n -> n % 2 == 0;
        
        // Method references
        List<String> names = Arrays.asList("Alice", "Bob", "Carol");
        names.forEach(System.out::println);
        names.stream().map(String::toUpperCase).toList();
        
        // Constructor reference
        Supplier<ArrayList<String>> listFactory = ArrayList::new;
    }
    
    // Stream API
    public static void streamExamples() {
        List<Integer> numbers = List.of(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
        
        // Filter, map, collect
        List<Integer> evenSquares = numbers.stream()
            .filter(n -> n % 2 == 0)
            .map(n -> n * n)
            .collect(Collectors.toList());
        
        // Reduce
        int sum = numbers.stream()
            .reduce(0, Integer::sum);
        
        // Statistics
        IntSummaryStatistics stats = numbers.stream()
            .mapToInt(Integer::intValue)
            .summaryStatistics();
        
        // Grouping
        Map<Boolean, List<Integer>> partitioned = numbers.stream()
            .collect(Collectors.partitioningBy(n -> n % 2 == 0));
        
        // Parallel stream
        long count = numbers.parallelStream()
            .filter(n -> n > 5)
            .count();
    }
    
    // CompletableFuture
    public static CompletableFuture<String> asyncOperation() {
        return CompletableFuture.supplyAsync(() -> {
            try {
                Thread.sleep(1000);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
            return "Result";
        }).thenApply(String::toUpperCase)
          .exceptionally(ex -> "Error: " + ex.getMessage());
    }
    
    // Try-with-resources
    public static void fileOperations() throws IOException {
        Path path = Paths.get("test.txt");
        
        // Writing
        try (BufferedWriter writer = Files.newBufferedWriter(path)) {
            writer.write("Hello, World!");
        }
        
        // Reading
        try (BufferedReader reader = Files.newBufferedReader(path);
             Stream<String> lines = reader.lines()) {
            lines.forEach(System.out::println);
        }
    }
    
    // Synchronized method
    public synchronized void synchronizedMethod() {
        // Thread-safe operation
    }
    
    // Main method
    public static void main(String[] args) {
        // Text blocks (Java 15+)
        String json = """
            {
                "name": "Java",
                "version": 21,
                "features": [
                    "Records",
                    "Sealed Classes",
                    "Pattern Matching"
                ]
            }
            """;
        
        System.out.println(json);
        
        // var keyword (Java 10+)
        var sample = new Sample("Test");
        var numbers = List.of(1, 2, 3, 4, 5);
        var map = new HashMap<String, Integer>();
        
        // Records
        var point = new Point(10, 20);
        var origin = Point.origin();
        System.out.println("Distance: " + point.distanceTo(origin));
        
        // Pattern matching
        Shape shape = new Circle(5.0);
        double area = calculateArea(shape);
        System.out.println("Area: " + area);
        
        // Stream operations
        streamExamples();
        lambdaExamples();
        
        // Async operation
        asyncOperation()
            .thenAccept(System.out::println)
            .join();
        
        // Exception handling
        try {
            throw new IllegalStateException("Test exception");
        } catch (IllegalStateException | IllegalArgumentException e) {
            System.err.println("Caught: " + e.getMessage());
        } finally {
            System.out.println("Cleanup");
        }
        
        // Switch expression
        DayOfWeek today = LocalDate.now().getDayOfWeek();
        System.out.println("Today is a " + dayType(today));
    }
}

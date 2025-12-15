/**
 * C++ Syntax Highlighting Test
 * 
 * This file demonstrates various C++ syntax constructs.
 */

#include <iostream>
#include <string>
#include <vector>
#include <map>
#include <memory>
#include <optional>
#include <variant>
#include <functional>
#include <algorithm>
#include <ranges>
#include <concepts>
#include <coroutine>
#include <format>

// Preprocessor
#define VERSION "1.0.0"
#define LOG(msg) std::cout << "[LOG] " << msg << std::endl

// Namespace
namespace syntax {
namespace detail {

// Constants
constexpr double PI = 3.14159265359;
constexpr int MAX_SIZE = 1024;

// Concepts (C++20)
template<typename T>
concept Numeric = std::integral<T> || std::floating_point<T>;

template<typename T>
concept Printable = requires(T t) {
    { std::cout << t } -> std::same_as<std::ostream&>;
};

// Enum class
enum class Color : uint8_t {
    Red = 1,
    Green = 2,
    Blue = 4
};

// Strongly typed enum
enum class Status {
    Pending,
    Active,
    Completed,
    Failed
};

constexpr std::string_view to_string(Status s) {
    switch (s) {
        case Status::Pending: return "pending";
        case Status::Active: return "active";
        case Status::Completed: return "completed";
        case Status::Failed: return "failed";
    }
    return "unknown";
}

// Forward declarations
class Person;
class Container;

// Abstract base class
class Shape {
public:
    virtual ~Shape() = default;
    virtual double area() const = 0;
    virtual double perimeter() const = 0;
    virtual void draw() const = 0;
};

// Interface using pure virtual class
class Serializable {
public:
    virtual ~Serializable() = default;
    virtual std::string serialize() const = 0;
    virtual void deserialize(std::string_view data) = 0;
};

// Concrete class with inheritance
class Rectangle : public Shape, public Serializable {
private:
    double width_;
    double height_;

public:
    // Constructors
    Rectangle() : width_(0), height_(0) {}
    Rectangle(double w, double h) : width_(w), height_(h) {}
    
    // Copy constructor
    Rectangle(const Rectangle& other) = default;
    
    // Move constructor
    Rectangle(Rectangle&& other) noexcept = default;
    
    // Copy assignment
    Rectangle& operator=(const Rectangle& other) = default;
    
    // Move assignment
    Rectangle& operator=(Rectangle&& other) noexcept = default;
    
    // Destructor
    ~Rectangle() override = default;
    
    // Getters and setters
    [[nodiscard]] double width() const { return width_; }
    [[nodiscard]] double height() const { return height_; }
    void set_width(double w) { width_ = w; }
    void set_height(double h) { height_ = h; }
    
    // Shape interface implementation
    [[nodiscard]] double area() const override {
        return width_ * height_;
    }
    
    [[nodiscard]] double perimeter() const override {
        return 2 * (width_ + height_);
    }
    
    void draw() const override {
        std::cout << "Drawing rectangle " << width_ << "x" << height_ << '\n';
    }
    
    // Serializable interface implementation
    [[nodiscard]] std::string serialize() const override {
        return std::format("Rectangle({}, {})", width_, height_);
    }
    
    void deserialize(std::string_view data) override {
        // Parse data...
    }
    
    // Operator overloading
    bool operator==(const Rectangle& other) const {
        return width_ == other.width_ && height_ == other.height_;
    }
    
    auto operator<=>(const Rectangle& other) const = default;
    
    friend std::ostream& operator<<(std::ostream& os, const Rectangle& r) {
        return os << "Rectangle(" << r.width_ << ", " << r.height_ << ")";
    }
};

// Template class
template<typename T>
class Container {
private:
    std::vector<T> items_;
    mutable std::mutex mutex_;

public:
    Container() = default;
    
    explicit Container(std::initializer_list<T> init) : items_(init) {}
    
    void add(const T& item) {
        std::lock_guard lock(mutex_);
        items_.push_back(item);
    }
    
    void add(T&& item) {
        std::lock_guard lock(mutex_);
        items_.push_back(std::move(item));
    }
    
    template<typename... Args>
    void emplace(Args&&... args) {
        std::lock_guard lock(mutex_);
        items_.emplace_back(std::forward<Args>(args)...);
    }
    
    [[nodiscard]] std::optional<T> get(size_t index) const {
        std::lock_guard lock(mutex_);
        if (index >= items_.size()) {
            return std::nullopt;
        }
        return items_[index];
    }
    
    [[nodiscard]] size_t size() const {
        std::lock_guard lock(mutex_);
        return items_.size();
    }
    
    // Range-based iteration
    auto begin() { return items_.begin(); }
    auto end() { return items_.end(); }
    auto begin() const { return items_.cbegin(); }
    auto end() const { return items_.cend(); }
};

// Template specialization
template<>
class Container<std::string> {
private:
    std::vector<std::string> items_;

public:
    void add(std::string_view item) {
        items_.emplace_back(item);
    }
    
    [[nodiscard]] size_t total_length() const {
        size_t total = 0;
        for (const auto& s : items_) {
            total += s.length();
        }
        return total;
    }
};

// Variadic template
template<typename... Args>
void print(Args&&... args) {
    (std::cout << ... << args) << '\n';
}

// Fold expression
template<Numeric... Nums>
auto sum(Nums... nums) {
    return (nums + ...);
}

// CRTP (Curiously Recurring Template Pattern)
template<typename Derived>
class Cloneable {
public:
    std::unique_ptr<Derived> clone() const {
        return std::make_unique<Derived>(static_cast<const Derived&>(*this));
    }
};

// Lambda expressions
auto multiply = [](auto a, auto b) { return a * b; };
auto is_even = [](int n) { return n % 2 == 0; };

// Constexpr function
constexpr int factorial(int n) {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}

// Consteval (C++20)
consteval int compile_time_square(int n) {
    return n * n;
}

// Function template with concepts
template<Numeric T>
T square(T value) {
    return value * value;
}

// Coroutine (C++20)
struct Task {
    struct promise_type {
        Task get_return_object() { return {}; }
        std::suspend_never initial_suspend() { return {}; }
        std::suspend_never final_suspend() noexcept { return {}; }
        void return_void() {}
        void unhandled_exception() {}
    };
};

Task async_task() {
    co_return;
}

} // namespace detail

// Using declarations
using detail::Rectangle;
using detail::Container;
using detail::Status;
using detail::Color;

} // namespace syntax

// Main function
int main(int argc, char* argv[]) {
    using namespace syntax;
    using namespace syntax::detail;
    
    // Modern C++ features demo
    
    // auto type deduction
    auto number = 42;
    auto pi = 3.14;
    auto name = std::string("C++");
    
    // Structured bindings (C++17)
    auto [x, y] = std::make_pair(10, 20);
    
    // Range-based for with initializer (C++20)
    for (std::vector v = {1, 2, 3, 4, 5}; auto& n : v) {
        n *= 2;
    }
    
    // if/switch with initializer (C++17)
    if (auto result = some_function(); result > 0) {
        std::cout << "Positive: " << result << '\n';
    }
    
    // std::optional
    std::optional<int> maybe_value;
    if (!maybe_value) {
        maybe_value = 42;
    }
    std::cout << "Value: " << maybe_value.value_or(-1) << '\n';
    
    // std::variant
    std::variant<int, double, std::string> var = "hello";
    std::visit([](auto&& arg) {
        std::cout << "Variant contains: " << arg << '\n';
    }, var);
    
    // Smart pointers
    auto rect = std::make_unique<Rectangle>(10.0, 5.0);
    auto shared_rect = std::make_shared<Rectangle>(20.0, 10.0);
    std::weak_ptr<Rectangle> weak_rect = shared_rect;
    
    // Algorithms with ranges (C++20)
    std::vector numbers = {5, 2, 8, 1, 9, 3, 7, 4, 6};
    
    auto even_squares = numbers
        | std::views::filter(is_even)
        | std::views::transform([](int n) { return n * n; });
    
    for (int n : even_squares) {
        std::cout << n << ' ';
    }
    std::cout << '\n';
    
    // Container usage
    Container<Rectangle> shapes;
    shapes.emplace(10.0, 5.0);
    shapes.emplace(20.0, 10.0);
    
    for (const auto& shape : shapes) {
        shape.draw();
    }
    
    // Constexpr evaluation
    constexpr int fact5 = factorial(5);
    constexpr int sq10 = compile_time_square(10);
    
    // String formatting (C++20)
    std::string formatted = std::format("Hello, {}! Pi = {:.2f}", name, pi);
    std::cout << formatted << '\n';
    
    // Exception handling
    try {
        throw std::runtime_error("Something went wrong");
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << '\n';
    }
    
    // RAII with scope guard
    {
        auto cleanup = []() { std::cout << "Cleanup!\n"; };
        // cleanup will be called when scope exits
    }
    
    return 0;
}

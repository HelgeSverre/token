// Package main demonstrates Go syntax highlighting
package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"sync"
	"time"
)

// Constants
const (
	MaxSize       = 1024
	DefaultPort   = 8080
	Pi            = 3.14159265359
	Greeting      = "Hello, World!"
	MultilineStr  = `This is a
multiline raw string
literal`
)

// Type aliases and definitions
type (
	ID     = string
	UserID int64
	Callback func(data any) error
)

// Enum-like constants with iota
type Status int

const (
	StatusPending Status = iota
	StatusActive
	StatusCompleted
	StatusFailed
)

func (s Status) String() string {
	switch s {
	case StatusPending:
		return "pending"
	case StatusActive:
		return "active"
	case StatusCompleted:
		return "completed"
	case StatusFailed:
		return "failed"
	default:
		return "unknown"
	}
}

// Interfaces
type Reader interface {
	Read(p []byte) (n int, err error)
}

type Writer interface {
	Write(p []byte) (n int, err error)
}

type ReadWriter interface {
	Reader
	Writer
}

type Stringer interface {
	String() string
}

// Structs
type Person struct {
	ID        UserID    `json:"id"`
	Name      string    `json:"name"`
	Email     string    `json:"email,omitempty"`
	Age       int       `json:"age"`
	Active    bool      `json:"active"`
	CreatedAt time.Time `json:"created_at"`
	tags      []string  // unexported field
}

// Constructor
func NewPerson(name string, age int) *Person {
	return &Person{
		ID:        UserID(time.Now().UnixNano()),
		Name:      name,
		Age:       age,
		Active:    true,
		CreatedAt: time.Now(),
	}
}

// Methods with pointer receiver
func (p *Person) SetEmail(email string) {
	p.Email = email
}

func (p *Person) AddTag(tag string) {
	p.tags = append(p.tags, tag)
}

// Method with value receiver
func (p Person) Greet() string {
	return fmt.Sprintf("Hello, I'm %s!", p.Name)
}

func (p Person) IsAdult() bool {
	return p.Age >= 18
}

// Implement Stringer interface
func (p Person) String() string {
	return fmt.Sprintf("Person{Name: %s, Age: %d}", p.Name, p.Age)
}

// Generic struct
type Container[T any] struct {
	items []T
	mu    sync.RWMutex
}

func NewContainer[T any]() *Container[T] {
	return &Container[T]{
		items: make([]T, 0),
	}
}

func (c *Container[T]) Add(item T) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.items = append(c.items, item)
}

func (c *Container[T]) Get(index int) (T, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()
	var zero T
	if index < 0 || index >= len(c.items) {
		return zero, false
	}
	return c.items[index], true
}

func (c *Container[T]) Len() int {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return len(c.items)
}

// Generic function
func Map[T, U any](items []T, fn func(T) U) []U {
	result := make([]U, len(items))
	for i, item := range items {
		result[i] = fn(item)
	}
	return result
}

func Filter[T any](items []T, predicate func(T) bool) []T {
	result := make([]T, 0)
	for _, item := range items {
		if predicate(item) {
			result = append(result, item)
		}
	}
	return result
}

func Reduce[T, U any](items []T, initial U, fn func(U, T) U) U {
	result := initial
	for _, item := range items {
		result = fn(result, item)
	}
	return result
}

// Error handling
var (
	ErrNotFound     = errors.New("not found")
	ErrUnauthorized = errors.New("unauthorized")
	ErrInvalidInput = errors.New("invalid input")
)

type ValidationError struct {
	Field   string
	Message string
}

func (e *ValidationError) Error() string {
	return fmt.Sprintf("validation error on %s: %s", e.Field, e.Message)
}

func ValidatePerson(p *Person) error {
	if p.Name == "" {
		return &ValidationError{Field: "name", Message: "cannot be empty"}
	}
	if p.Age < 0 {
		return &ValidationError{Field: "age", Message: "cannot be negative"}
	}
	return nil
}

// HTTP handler
func handleUsers(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		users := []Person{
			{Name: "Alice", Age: 30},
			{Name: "Bob", Age: 25},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(users)
	case http.MethodPost:
		var person Person
		if err := json.NewDecoder(r.Body).Decode(&person); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		if err := ValidatePerson(&person); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		w.WriteHeader(http.StatusCreated)
		json.NewEncoder(w).Encode(person)
	default:
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
	}
}

// Context and cancellation
func fetchData(ctx context.Context, url string) ([]byte, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, fmt.Errorf("creating request: %w", err)
	}

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("executing request: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("unexpected status: %d", resp.StatusCode)
	}

	return io.ReadAll(resp.Body)
}

// Goroutines and channels
func worker(id int, jobs <-chan int, results chan<- int, wg *sync.WaitGroup) {
	defer wg.Done()
	for job := range jobs {
		fmt.Printf("Worker %d processing job %d\n", id, job)
		time.Sleep(100 * time.Millisecond)
		results <- job * 2
	}
}

func runWorkerPool(numWorkers, numJobs int) []int {
	jobs := make(chan int, numJobs)
	results := make(chan int, numJobs)
	var wg sync.WaitGroup

	// Start workers
	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go worker(i, jobs, results, &wg)
	}

	// Send jobs
	for j := 0; j < numJobs; j++ {
		jobs <- j
	}
	close(jobs)

	// Wait and collect results
	go func() {
		wg.Wait()
		close(results)
	}()

	var output []int
	for result := range results {
		output = append(output, result)
	}
	return output
}

// Select statement
func timeout(ch <-chan string, duration time.Duration) (string, error) {
	select {
	case msg := <-ch:
		return msg, nil
	case <-time.After(duration):
		return "", errors.New("timeout")
	}
}

// Defer, panic, recover
func safeOperation() (err error) {
	defer func() {
		if r := recover(); r != nil {
			err = fmt.Errorf("recovered from panic: %v", r)
		}
	}()

	// Some risky operation
	panic("something went wrong")
}

// Main function
func main() {
	// Variables
	var name string = "Go"
	age := 15
	pi := 3.14
	active := true

	// Print
	fmt.Printf("Language: %s, Age: %d, Pi: %.2f, Active: %t\n",
		name, age, pi, active)

	// Slices and maps
	numbers := []int{1, 2, 3, 4, 5}
	doubled := Map(numbers, func(n int) int { return n * 2 })
	fmt.Println("Doubled:", doubled)

	scores := map[string]int{
		"alice": 100,
		"bob":   85,
		"carol": 92,
	}
	for name, score := range scores {
		fmt.Printf("%s: %d\n", name, score)
	}

	// Create person
	person := NewPerson("Alice", 30)
	person.SetEmail("alice@example.com")
	fmt.Println(person.Greet())

	// Generic container
	container := NewContainer[string]()
	container.Add("hello")
	container.Add("world")
	fmt.Printf("Container has %d items\n", container.Len())

	// Error handling
	if err := ValidatePerson(person); err != nil {
		log.Printf("Validation failed: %v", err)
	}

	// Start HTTP server
	http.HandleFunc("/users", handleUsers)
	log.Println("Starting server on :8080")
	if err := http.ListenAndServe(":8080", nil); err != nil {
		log.Fatal(err)
	}
}

// Init function
func init() {
	log.SetFlags(log.LstdFlags | log.Lshortfile)
}

/**
 * C Syntax Highlighting Test
 * 
 * This file demonstrates various C syntax constructs.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <stdint.h>
#include <math.h>

/* Preprocessor definitions */
#define VERSION "1.0.0"
#define MAX_SIZE 1024
#define MIN(a, b) ((a) < (b) ? (a) : (b))
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define ARRAY_SIZE(arr) (sizeof(arr) / sizeof((arr)[0]))
#define STRINGIFY(x) #x
#define CONCAT(a, b) a##b

#ifdef DEBUG
    #define LOG(fmt, ...) printf("[DEBUG] " fmt "\n", ##__VA_ARGS__)
#else
    #define LOG(fmt, ...) ((void)0)
#endif

#ifndef PI
    #define PI 3.14159265359
#endif

/* Type definitions */
typedef unsigned char byte;
typedef uint32_t uint;
typedef int (*compare_fn)(const void*, const void*);
typedef void (*callback_fn)(void* data);

/* Enumerations */
enum Color {
    COLOR_RED = 1,
    COLOR_GREEN = 2,
    COLOR_BLUE = 4,
    COLOR_ALL = COLOR_RED | COLOR_GREEN | COLOR_BLUE
};

typedef enum {
    STATUS_PENDING,
    STATUS_ACTIVE,
    STATUS_COMPLETED,
    STATUS_FAILED
} Status;

/* Structures */
struct Point {
    int x;
    int y;
};

typedef struct {
    char name[64];
    int age;
    float score;
    bool active;
    struct Point location;
} Person;

/* Unions */
typedef union {
    int as_int;
    float as_float;
    char as_bytes[4];
} Value;

/* Bit fields */
typedef struct {
    unsigned int flag1 : 1;
    unsigned int flag2 : 1;
    unsigned int flag3 : 1;
    unsigned int reserved : 5;
    unsigned int value : 8;
} Flags;

/* Function prototypes */
void print_person(const Person* p);
Person* create_person(const char* name, int age);
void free_person(Person* p);
int compare_persons(const void* a, const void* b);

/* Static and extern variables */
static int instance_count = 0;
extern int global_flag;

/* Constants */
const double E = 2.71828182845;
static const char* GREETING = "Hello, World!";

/* Inline function */
static inline int square(int x) {
    return x * x;
}

/* Function with variadic arguments */
void log_message(const char* format, ...) {
    va_list args;
    va_start(args, format);
    vprintf(format, args);
    va_end(args);
    printf("\n");
}

/* Create person */
Person* create_person(const char* name, int age) {
    Person* p = (Person*)malloc(sizeof(Person));
    if (p == NULL) {
        return NULL;
    }
    
    strncpy(p->name, name, sizeof(p->name) - 1);
    p->name[sizeof(p->name) - 1] = '\0';
    p->age = age;
    p->score = 0.0f;
    p->active = true;
    p->location.x = 0;
    p->location.y = 0;
    
    instance_count++;
    return p;
}

/* Free person */
void free_person(Person* p) {
    if (p != NULL) {
        free(p);
        instance_count--;
    }
}

/* Print person */
void print_person(const Person* p) {
    if (p == NULL) {
        printf("NULL person\n");
        return;
    }
    
    printf("Person {\n");
    printf("  name: %s\n", p->name);
    printf("  age: %d\n", p->age);
    printf("  score: %.2f\n", p->score);
    printf("  active: %s\n", p->active ? "true" : "false");
    printf("  location: (%d, %d)\n", p->location.x, p->location.y);
    printf("}\n");
}

/* Compare persons for sorting */
int compare_persons(const void* a, const void* b) {
    const Person* pa = (const Person*)a;
    const Person* pb = (const Person*)b;
    return strcmp(pa->name, pb->name);
}

/* Generic linked list node */
typedef struct Node {
    void* data;
    struct Node* next;
} Node;

/* Linked list operations */
Node* list_create(void* data) {
    Node* node = (Node*)malloc(sizeof(Node));
    if (node) {
        node->data = data;
        node->next = NULL;
    }
    return node;
}

void list_append(Node** head, void* data) {
    Node* new_node = list_create(data);
    if (*head == NULL) {
        *head = new_node;
        return;
    }
    
    Node* current = *head;
    while (current->next != NULL) {
        current = current->next;
    }
    current->next = new_node;
}

void list_foreach(Node* head, callback_fn callback) {
    Node* current = head;
    while (current != NULL) {
        callback(current->data);
        current = current->next;
    }
}

void list_free(Node* head, callback_fn free_data) {
    Node* current = head;
    while (current != NULL) {
        Node* next = current->next;
        if (free_data) {
            free_data(current->data);
        }
        free(current);
        current = next;
    }
}

/* Array utilities */
void swap(int* a, int* b) {
    int temp = *a;
    *a = *b;
    *b = temp;
}

void bubble_sort(int arr[], size_t n) {
    for (size_t i = 0; i < n - 1; i++) {
        for (size_t j = 0; j < n - i - 1; j++) {
            if (arr[j] > arr[j + 1]) {
                swap(&arr[j], &arr[j + 1]);
            }
        }
    }
}

int binary_search(const int arr[], size_t n, int target) {
    size_t left = 0;
    size_t right = n - 1;
    
    while (left <= right) {
        size_t mid = left + (right - left) / 2;
        
        if (arr[mid] == target) {
            return (int)mid;
        } else if (arr[mid] < target) {
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }
    
    return -1;
}

/* String utilities */
char* string_duplicate(const char* str) {
    if (str == NULL) {
        return NULL;
    }
    
    size_t len = strlen(str);
    char* dup = (char*)malloc(len + 1);
    if (dup) {
        strcpy(dup, str);
    }
    return dup;
}

void string_reverse(char* str) {
    if (str == NULL || *str == '\0') {
        return;
    }
    
    char* start = str;
    char* end = str + strlen(str) - 1;
    
    while (start < end) {
        char temp = *start;
        *start++ = *end;
        *end-- = temp;
    }
}

/* Main function */
int main(int argc, char* argv[]) {
    // Print arguments
    printf("Arguments (%d):\n", argc);
    for (int i = 0; i < argc; i++) {
        printf("  [%d] %s\n", i, argv[i]);
    }
    
    // Different number formats
    int decimal = 255;
    int hex = 0xFF;
    int octal = 0377;
    int binary = 0b11111111;  // C23 or GCC extension
    float pi = 3.14159f;
    double e = 2.71828;
    
    printf("Decimal: %d, Hex: %d, Octal: %d\n", decimal, hex, octal);
    
    // Create person
    Person* alice = create_person("Alice", 30);
    if (alice) {
        alice->score = 95.5f;
        alice->location.x = 10;
        alice->location.y = 20;
        print_person(alice);
        free_person(alice);
    }
    
    // Arrays
    int numbers[] = {5, 2, 8, 1, 9, 3, 7, 4, 6};
    size_t count = ARRAY_SIZE(numbers);
    
    bubble_sort(numbers, count);
    printf("Sorted: ");
    for (size_t i = 0; i < count; i++) {
        printf("%d ", numbers[i]);
    }
    printf("\n");
    
    // Search
    int index = binary_search(numbers, count, 5);
    printf("Found 5 at index: %d\n", index);
    
    // Strings
    char* str = string_duplicate("Hello, World!");
    printf("Original: %s\n", str);
    string_reverse(str);
    printf("Reversed: %s\n", str);
    free(str);
    
    // Unions
    Value v;
    v.as_int = 1078530011;
    printf("As int: %d, As float: %f\n", v.as_int, v.as_float);
    
    // Switch statement
    Status status = STATUS_ACTIVE;
    switch (status) {
        case STATUS_PENDING:
            printf("Pending\n");
            break;
        case STATUS_ACTIVE:
            printf("Active\n");
            break;
        case STATUS_COMPLETED:
            printf("Completed\n");
            break;
        case STATUS_FAILED:
            printf("Failed\n");
            break;
        default:
            printf("Unknown\n");
    }
    
    // Conditional operator
    int max = (decimal > hex) ? decimal : hex;
    printf("Max: %d\n", max);
    
    // Pointers
    int value = 42;
    int* ptr = &value;
    int** pptr = &ptr;
    printf("Value: %d, *ptr: %d, **pptr: %d\n", value, *ptr, **pptr);
    
    // Function pointers
    compare_fn cmp = compare_persons;
    
    // Goto (rarely used, but valid)
    goto cleanup;
    
cleanup:
    printf("Cleanup done\n");
    
    return EXIT_SUCCESS;
}

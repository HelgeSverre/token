#!/usr/bin/env python3
"""
Python Syntax Highlighting Test

This module demonstrates various Python syntax constructs.
"""

from __future__ import annotations

import asyncio
import json
import os
import re
import sys
from abc import ABC, abstractmethod
from collections import defaultdict, namedtuple
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum, auto
from functools import lru_cache, wraps
from pathlib import Path
from typing import (
    Any,
    Callable,
    Dict,
    Generic,
    Iterator,
    List,
    Literal,
    Optional,
    Protocol,
    Tuple,
    TypeVar,
    Union,
    overload,
)

# Type variables
T = TypeVar('T')
K = TypeVar('K')
V = TypeVar('V')

# Constants
PI: float = 3.14159265359
MAX_SIZE: int = 1_000_000
DEBUG: bool = True
GREETING: str = "Hello, World!"

# Different number formats
decimal = 1_000_000
hexadecimal = 0xDEADBEEF
octal = 0o755
binary = 0b10101010
scientific = 1.5e-10
complex_num = 3 + 4j

# Strings
single_quoted = 'Single quotes'
double_quoted = "Double quotes"
raw_string = r"Raw \n string"
bytes_literal = b"Bytes literal"
f_string = f"Interpolated: {PI:.2f}"
multiline = """
    Multi-line
    string
"""


# Enums
class Color(Enum):
    RED = auto()
    GREEN = auto()
    BLUE = auto()

    def __str__(self) -> str:
        return self.name.lower()


class Status(Enum):
    PENDING = "pending"
    ACTIVE = "active"
    COMPLETED = "completed"


# Named tuple
Point = namedtuple('Point', ['x', 'y'])


# Dataclass
@dataclass
class Person:
    name: str
    age: int
    email: Optional[str] = None
    tags: List[str] = field(default_factory=list)
    created_at: datetime = field(default_factory=datetime.now)

    def __post_init__(self) -> None:
        if self.age < 0:
            raise ValueError("Age cannot be negative")

    @property
    def is_adult(self) -> bool:
        return self.age >= 18

    def greet(self) -> str:
        return f"Hello, I'm {self.name}!"


# Protocol (structural typing)
class Comparable(Protocol):
    def __lt__(self, other: Any) -> bool: ...
    def __eq__(self, other: Any) -> bool: ...


# Abstract base class
class Shape(ABC):
    @abstractmethod
    def area(self) -> float:
        pass

    @abstractmethod
    def perimeter(self) -> float:
        pass

    def describe(self) -> str:
        return f"Area: {self.area():.2f}, Perimeter: {self.perimeter():.2f}"


class Rectangle(Shape):
    def __init__(self, width: float, height: float) -> None:
        self._width = width
        self._height = height

    @property
    def width(self) -> float:
        return self._width

    @width.setter
    def width(self, value: float) -> None:
        if value <= 0:
            raise ValueError("Width must be positive")
        self._width = value

    def area(self) -> float:
        return self._width * self._height

    def perimeter(self) -> float:
        return 2 * (self._width + self._height)


class Circle(Shape):
    def __init__(self, radius: float) -> None:
        self.radius = radius

    def area(self) -> float:
        return PI * self.radius ** 2

    def perimeter(self) -> float:
        return 2 * PI * self.radius


# Generic class
class Container(Generic[T]):
    def __init__(self) -> None:
        self._items: List[T] = []

    def add(self, item: T) -> None:
        self._items.append(item)

    def get(self, index: int) -> T:
        return self._items[index]

    def __iter__(self) -> Iterator[T]:
        return iter(self._items)

    def __len__(self) -> int:
        return len(self._items)


# Decorators
def timer(func: Callable[..., T]) -> Callable[..., T]:
    @wraps(func)
    def wrapper(*args: Any, **kwargs: Any) -> T:
        import time
        start = time.perf_counter()
        result = func(*args, **kwargs)
        end = time.perf_counter()
        print(f"{func.__name__} took {end - start:.4f}s")
        return result
    return wrapper


def retry(max_attempts: int = 3, delay: float = 1.0):
    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        @wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> T:
            for attempt in range(max_attempts):
                try:
                    return func(*args, **kwargs)
                except Exception as e:
                    if attempt == max_attempts - 1:
                        raise
                    import time
                    time.sleep(delay)
            raise RuntimeError("Should not reach here")
        return wrapper
    return decorator


# Function overloads
@overload
def process(value: str) -> str: ...
@overload
def process(value: int) -> int: ...
@overload
def process(value: list) -> list: ...


def process(value: Union[str, int, list]) -> Union[str, int, list]:
    if isinstance(value, str):
        return value.upper()
    elif isinstance(value, int):
        return value * 2
    else:
        return list(reversed(value))


# Cached function
@lru_cache(maxsize=128)
def fibonacci(n: int) -> int:
    if n < 2:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)


# Generator function
def count_up_to(n: int) -> Iterator[int]:
    i = 0
    while i < n:
        yield i
        i += 1


# Async functions
async def fetch_data(url: str) -> dict:
    await asyncio.sleep(0.1)  # Simulate network delay
    return {"url": url, "data": "sample"}


async def main() -> None:
    tasks = [fetch_data(f"https://api.example.com/{i}") for i in range(5)]
    results = await asyncio.gather(*tasks)
    for result in results:
        print(result)


# Context manager
class FileManager:
    def __init__(self, filename: str, mode: str = 'r') -> None:
        self.filename = filename
        self.mode = mode
        self.file = None

    def __enter__(self):
        self.file = open(self.filename, self.mode)
        return self.file

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.file:
            self.file.close()
        return False


# Pattern matching (Python 3.10+)
def http_status(status: int) -> str:
    match status:
        case 200:
            return "OK"
        case 201:
            return "Created"
        case 400:
            return "Bad Request"
        case 404:
            return "Not Found"
        case 500 | 502 | 503:
            return "Server Error"
        case _:
            return "Unknown"


def process_command(command: dict) -> str:
    match command:
        case {"action": "create", "name": name}:
            return f"Creating {name}"
        case {"action": "delete", "id": id}:
            return f"Deleting {id}"
        case {"action": action, **rest}:
            return f"Unknown action: {action}"
        case _:
            return "Invalid command"


# List comprehensions
squares = [x ** 2 for x in range(10)]
evens = [x for x in range(20) if x % 2 == 0]
matrix = [[i * j for j in range(5)] for i in range(5)]

# Dict and set comprehensions
word_lengths = {word: len(word) for word in ["hello", "world"]}
unique_chars = {char for char in "hello world"}

# Generator expression
sum_of_squares = sum(x ** 2 for x in range(1000))


# Exception handling
class CustomError(Exception):
    def __init__(self, message: str, code: int) -> None:
        super().__init__(message)
        self.code = code


def risky_operation(value: int) -> int:
    try:
        if value < 0:
            raise ValueError("Negative value")
        if value == 0:
            raise CustomError("Zero is not allowed", code=400)
        return 100 // value
    except ValueError as e:
        print(f"Value error: {e}")
        raise
    except CustomError as e:
        print(f"Custom error (code {e.code}): {e}")
        raise
    except ZeroDivisionError:
        print("Division by zero")
        return 0
    finally:
        print("Cleanup")


# Lambda functions
add = lambda x, y: x + y
double = lambda x: x * 2
is_even = lambda x: x % 2 == 0

# Higher-order functions
numbers = [1, 2, 3, 4, 5]
doubled = list(map(lambda x: x * 2, numbers))
filtered = list(filter(lambda x: x > 2, numbers))
from functools import reduce
total = reduce(lambda a, b: a + b, numbers, 0)


# Main entry point
if __name__ == "__main__":
    # Create instances
    person = Person("Alice", 30, "alice@example.com")
    rect = Rectangle(10, 5)
    circle = Circle(7)

    # Use pattern matching
    status = http_status(200)
    print(f"Status: {status}")

    # Run async
    asyncio.run(main())

/**
 * TypeScript Syntax Highlighting Test
 * This file demonstrates various TypeScript constructs.
 */

// Type aliases
type ID = string | number;
type Nullable<T> = T | null;
type Callback<T> = (data: T) => void;
type RecursivePartial<T> = { [K in keyof T]?: RecursivePartial<T[K]> };

// Interfaces
interface User {
    readonly id: ID;
    name: string;
    email: string;
    age?: number;
    roles: string[];
    metadata: Record<string, unknown>;
}

interface ApiResponse<T> {
    data: T;
    status: number;
    message?: string;
}

// Extending interfaces
interface Admin extends User {
    permissions: string[];
    level: 'super' | 'regular';
}

// Enums
enum Direction {
    Up = 'UP',
    Down = 'DOWN',
    Left = 'LEFT',
    Right = 'RIGHT',
}

const enum HttpStatus {
    OK = 200,
    Created = 201,
    BadRequest = 400,
    NotFound = 404,
    ServerError = 500,
}

// Classes with access modifiers
class Person {
    private _id: string;
    protected name: string;
    public age: number;
    readonly createdAt: Date;
    static instanceCount: number = 0;

    constructor(name: string, age: number) {
        this._id = crypto.randomUUID();
        this.name = name;
        this.age = age;
        this.createdAt = new Date();
        Person.instanceCount++;
    }

    get id(): string {
        return this._id;
    }

    public greet(): string {
        return `Hello, I'm ${this.name}`;
    }

    protected formatAge(): string {
        return `${this.age} years old`;
    }

    private validateAge(): boolean {
        return this.age >= 0 && this.age <= 150;
    }
}

// Abstract classes
abstract class Shape {
    abstract area(): number;
    abstract perimeter(): number;
    
    describe(): string {
        return `Area: ${this.area()}, Perimeter: ${this.perimeter()}`;
    }
}

class Rectangle extends Shape {
    constructor(private width: number, private height: number) {
        super();
    }

    area(): number {
        return this.width * this.height;
    }

    perimeter(): number {
        return 2 * (this.width + this.height);
    }
}

// Generic classes
class Container<T> {
    private items: T[] = [];

    add(item: T): void {
        this.items.push(item);
    }

    get(index: number): T | undefined {
        return this.items[index];
    }

    getAll(): readonly T[] {
        return this.items;
    }
}

// Generic functions
function identity<T>(value: T): T {
    return value;
}

function map<T, U>(array: T[], fn: (item: T) => U): U[] {
    return array.map(fn);
}

function merge<T extends object, U extends object>(a: T, b: U): T & U {
    return { ...a, ...b };
}

// Generic constraints
function getProperty<T, K extends keyof T>(obj: T, key: K): T[K] {
    return obj[key];
}

// Conditional types
type IsArray<T> = T extends unknown[] ? true : false;
type Unwrap<T> = T extends Promise<infer U> ? U : T;
type ElementType<T> = T extends (infer E)[] ? E : never;

// Mapped types
type Readonly<T> = { readonly [K in keyof T]: T[K] };
type Optional<T> = { [K in keyof T]?: T[K] };
type Required<T> = { [K in keyof T]-?: T[K] };

// Template literal types
type EventName = `on${Capitalize<'click' | 'focus' | 'blur'>}`;
type Getter<T extends string> = `get${Capitalize<T>}`;
type Setter<T extends string> = `set${Capitalize<T>}`;

// Utility types
type PartialUser = Partial<User>;
type RequiredUser = Required<User>;
type ReadonlyUser = Readonly<User>;
type PickedUser = Pick<User, 'id' | 'name'>;
type OmittedUser = Omit<User, 'metadata'>;
type ExtractedString = Extract<ID, string>;
type ExcludedNumber = Exclude<ID, number>;
type NonNullableID = NonNullable<ID | null | undefined>;
type UserKeys = keyof User;
type UserValues = User[keyof User];

// Function types
type VoidFunction = () => void;
type AsyncFunction = () => Promise<void>;
type EventHandler = (event: Event) => void;

// Function overloads
function parse(input: string): number;
function parse(input: number): string;
function parse(input: string | number): string | number {
    if (typeof input === 'string') {
        return parseInt(input, 10);
    }
    return input.toString();
}

// Decorators (experimental)
function logged(target: any, key: string, descriptor: PropertyDescriptor) {
    const original = descriptor.value;
    descriptor.value = function (...args: any[]) {
        console.log(`Calling ${key} with`, args);
        return original.apply(this, args);
    };
    return descriptor;
}

// Type guards
function isString(value: unknown): value is string {
    return typeof value === 'string';
}

function isUser(value: unknown): value is User {
    return (
        typeof value === 'object' &&
        value !== null &&
        'id' in value &&
        'name' in value
    );
}

// Assertion functions
function assertDefined<T>(value: T | null | undefined): asserts value is T {
    if (value === null || value === undefined) {
        throw new Error('Value is not defined');
    }
}

// Async/await with types
async function fetchUser(id: ID): Promise<User> {
    const response = await fetch(`/api/users/${id}`);
    const data: ApiResponse<User> = await response.json();
    return data.data;
}

// Namespace
namespace Validation {
    export interface Validator {
        validate(value: string): boolean;
    }

    export class EmailValidator implements Validator {
        validate(value: string): boolean {
            return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);
        }
    }
}

// Module augmentation
declare module 'express' {
    interface Request {
        user?: User;
    }
}

// Ambient declarations
declare const VERSION: string;
declare function external(): void;
declare class ExternalClass {
    constructor(config: object);
}

// satisfies operator
const palette = {
    red: [255, 0, 0],
    green: '#00ff00',
    blue: [0, 0, 255],
} satisfies Record<string, string | number[]>;

// as const
const config = {
    endpoint: 'https://api.example.com',
    timeout: 5000,
    retries: 3,
} as const;

// Non-null assertion
function processInput(input: string | null) {
    const length = input!.length; // Assert non-null
}

// Export types
export type { User, Admin, ApiResponse };
export { Person, Container, Direction, HttpStatus };
export default Rectangle;

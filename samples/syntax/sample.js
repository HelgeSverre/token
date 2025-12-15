/**
 * JavaScript Syntax Highlighting Test
 * This file demonstrates various JavaScript constructs.
 */

'use strict';

// Imports
import { Component } from 'react';
import * as utils from './utils.js';
import defaultExport from './module.js';

// Variables
const PI = 3.14159265359;
let counter = 0;
var legacyVar = 'still works';

// Different number formats
const decimal = 1_000_000;
const hex = 0xDEADBEEF;
const octal = 0o755;
const binary = 0b10101010;
const scientific = 1.5e-10;
const bigInt = 9007199254740991n;

// Strings
const single = 'Single quotes';
const double = "Double quotes";
const template = `Template literal with ${counter} interpolation`;
const multiline = `
    Multi-line
    template
    string
`;
const tagged = String.raw`Raw \n string`;

// Regular expressions
const regex = /^[a-z]+$/gi;
const regexWithFlags = new RegExp('pattern', 'gim');

// Objects
const person = {
    name: 'John',
    age: 30,
    'hyphenated-key': 'value',
    [Symbol.toStringTag]: 'Person',
    greet() {
        return `Hello, I'm ${this.name}`;
    },
    get fullName() {
        return `${this.name} Doe`;
    },
    set fullName(value) {
        this.name = value.split(' ')[0];
    },
};

// Object destructuring
const { name, age, ...rest } = person;
const { name: personName = 'Anonymous' } = person;

// Arrays
const numbers = [1, 2, 3, 4, 5];
const mixed = [1, 'two', { three: 3 }, [4, 5]];
const [first, second, ...remaining] = numbers;

// Spread operator
const combined = [...numbers, ...mixed];
const cloned = { ...person, extra: 'property' };

// Classes
class Animal {
    #privateField = 'private';
    static count = 0;
    
    constructor(name) {
        this.name = name;
        Animal.count++;
    }
    
    speak() {
        console.log(`${this.name} makes a sound`);
    }
    
    static getCount() {
        return Animal.count;
    }
    
    get privateValue() {
        return this.#privateField;
    }
}

class Dog extends Animal {
    constructor(name, breed) {
        super(name);
        this.breed = breed;
    }
    
    speak() {
        console.log(`${this.name} barks`);
    }
    
    fetch() {
        return `${this.name} fetches the ball`;
    }
}

// Functions
function traditionalFunction(a, b) {
    return a + b;
}

function* generatorFunction() {
    yield 1;
    yield 2;
    yield 3;
}

async function asyncFunction() {
    const response = await fetch('/api/data');
    return response.json();
}

const arrowFunction = (x, y) => x + y;
const arrowWithBody = (x) => {
    const doubled = x * 2;
    return doubled;
};
const implicitReturn = x => x * x;

// Default and rest parameters
function withDefaults(a = 1, b = 2, c = a + b) {
    return a + b + c;
}

function withRest(first, ...others) {
    return others.reduce((sum, n) => sum + n, first);
}

// Higher-order functions
const doubled = numbers.map(n => n * 2);
const evens = numbers.filter(n => n % 2 === 0);
const sum = numbers.reduce((acc, n) => acc + n, 0);
const found = numbers.find(n => n > 3);
const hasNegative = numbers.some(n => n < 0);
const allPositive = numbers.every(n => n > 0);

// Control flow
if (counter === 0) {
    console.log('Zero');
} else if (counter > 0) {
    console.log('Positive');
} else {
    console.log('Negative');
}

switch (counter) {
    case 0:
        console.log('Zero');
        break;
    case 1:
    case 2:
        console.log('One or two');
        break;
    default:
        console.log('Other');
}

// Loops
for (let i = 0; i < 10; i++) {
    console.log(i);
}

for (const item of numbers) {
    console.log(item);
}

for (const key in person) {
    console.log(key, person[key]);
}

while (counter < 10) {
    counter++;
}

do {
    counter--;
} while (counter > 0);

// Try-catch
try {
    throw new Error('Something went wrong');
} catch (error) {
    console.error(error.message);
} finally {
    console.log('Cleanup');
}

// Promises
const promise = new Promise((resolve, reject) => {
    setTimeout(() => resolve('Done!'), 1000);
});

promise
    .then(result => console.log(result))
    .catch(error => console.error(error))
    .finally(() => console.log('Complete'));

// Async/await
async function fetchData() {
    try {
        const [users, posts] = await Promise.all([
            fetch('/api/users').then(r => r.json()),
            fetch('/api/posts').then(r => r.json()),
        ]);
        return { users, posts };
    } catch (error) {
        throw new Error(`Failed to fetch: ${error.message}`);
    }
}

// Proxy
const handler = {
    get(target, prop) {
        return prop in target ? target[prop] : 'default';
    },
    set(target, prop, value) {
        console.log(`Setting ${prop} to ${value}`);
        target[prop] = value;
        return true;
    },
};
const proxy = new Proxy({}, handler);

// Symbol
const sym = Symbol('description');
const globalSym = Symbol.for('global');

// Map and Set
const map = new Map([
    ['key1', 'value1'],
    ['key2', 'value2'],
]);
const set = new Set([1, 2, 3, 3, 3]);
const weakMap = new WeakMap();
const weakSet = new WeakSet();

// Nullish coalescing and optional chaining
const value = null ?? 'default';
const nested = person?.address?.city ?? 'Unknown';
const method = person.greet?.();

// Logical assignment
let x = null;
x ??= 'default';
x ||= 'fallback';
x &&= 'updated';

// Export
export { person, Animal, Dog };
export default class DefaultExport {}

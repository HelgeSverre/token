<?php
/**
 * PHP Syntax Highlighting Test
 * 
 * This file demonstrates various PHP syntax constructs.
 */

declare(strict_types=1);

namespace App\Syntax;

use App\Interfaces\Stringable;
use App\Traits\Loggable;
use DateTime;
use DateTimeImmutable;
use Exception;
use InvalidArgumentException;
use JsonSerializable;
use PDO;
use RuntimeException;

// Constants
const VERSION = '1.0.0';
const PI = 3.14159265359;
const DEBUG = true;

define('APP_NAME', 'Syntax Test');
define('MAX_SIZE', 1024);

// Enums (PHP 8.1+)
enum Status: string
{
    case Pending = 'pending';
    case Active = 'active';
    case Completed = 'completed';
    case Failed = 'failed';

    public function label(): string
    {
        return match($this) {
            self::Pending => 'Waiting',
            self::Active => 'In Progress',
            self::Completed => 'Done',
            self::Failed => 'Error',
        };
    }
}

enum Color: int
{
    case Red = 1;
    case Green = 2;
    case Blue = 3;
}

// Interface
interface UserInterface
{
    public function getId(): int;
    public function getName(): string;
    public function getEmail(): ?string;
}

// Trait
trait TimestampTrait
{
    protected ?DateTimeImmutable $createdAt = null;
    protected ?DateTimeImmutable $updatedAt = null;

    public function getCreatedAt(): ?DateTimeImmutable
    {
        return $this->createdAt;
    }

    public function setCreatedAt(DateTimeImmutable $createdAt): void
    {
        $this->createdAt = $createdAt;
    }

    public function touch(): void
    {
        $this->updatedAt = new DateTimeImmutable();
    }
}

// Abstract class
abstract class Entity
{
    protected int $id;

    abstract public function validate(): bool;

    public function getId(): int
    {
        return $this->id;
    }
}

// Class with attributes (PHP 8.0+)
#[Attribute]
class Route
{
    public function __construct(
        public string $path,
        public array $methods = ['GET'],
    ) {}
}

// Main class
class User extends Entity implements UserInterface, JsonSerializable
{
    use TimestampTrait;

    public const ROLE_USER = 'user';
    public const ROLE_ADMIN = 'admin';

    private static int $instanceCount = 0;

    // Constructor property promotion (PHP 8.0+)
    public function __construct(
        protected string $name,
        protected string $email,
        protected string $role = self::ROLE_USER,
        private ?string $password = null,
        public readonly bool $active = true,
    ) {
        $this->id = ++self::$instanceCount;
        $this->createdAt = new DateTimeImmutable();
    }

    // Getters
    public function getId(): int
    {
        return $this->id;
    }

    public function getName(): string
    {
        return $this->name;
    }

    public function getEmail(): ?string
    {
        return $this->email;
    }

    // Setters with fluent interface
    public function setName(string $name): self
    {
        $this->name = $name;
        $this->touch();
        return $this;
    }

    public function setEmail(string $email): self
    {
        if (!filter_var($email, FILTER_VALIDATE_EMAIL)) {
            throw new InvalidArgumentException("Invalid email: $email");
        }
        $this->email = $email;
        $this->touch();
        return $this;
    }

    // Method with union types (PHP 8.0+)
    public function setPassword(string|null $password): void
    {
        $this->password = $password ? password_hash($password, PASSWORD_DEFAULT) : null;
    }

    // Method with named arguments
    public function greet(string $greeting = 'Hello', bool $formal = false): string
    {
        $title = $formal ? 'Mr./Ms. ' : '';
        return "$greeting, {$title}{$this->name}!";
    }

    // Static method
    public static function getInstanceCount(): int
    {
        return self::$instanceCount;
    }

    // Validation
    public function validate(): bool
    {
        return !empty($this->name) && !empty($this->email);
    }

    // JsonSerializable implementation
    public function jsonSerialize(): array
    {
        return [
            'id' => $this->id,
            'name' => $this->name,
            'email' => $this->email,
            'role' => $this->role,
            'active' => $this->active,
            'created_at' => $this->createdAt?->format('c'),
        ];
    }

    // Magic methods
    public function __toString(): string
    {
        return "User#{$this->id}: {$this->name}";
    }

    public function __get(string $name): mixed
    {
        return $this->$name ?? null;
    }

    public function __isset(string $name): bool
    {
        return isset($this->$name);
    }
}

// Generic-like class using templates
/**
 * @template T
 */
class Container
{
    /** @var array<int, T> */
    private array $items = [];

    /**
     * @param T $item
     */
    public function add(mixed $item): void
    {
        $this->items[] = $item;
    }

    /**
     * @return T|null
     */
    public function get(int $index): mixed
    {
        return $this->items[$index] ?? null;
    }

    public function count(): int
    {
        return count($this->items);
    }

    /**
     * @return array<int, T>
     */
    public function all(): array
    {
        return $this->items;
    }
}

// Controller with attributes
#[Route('/users')]
class UserController
{
    public function __construct(
        private readonly UserRepository $repository,
    ) {}

    #[Route('/users', methods: ['GET'])]
    public function index(): array
    {
        return $this->repository->findAll();
    }

    #[Route('/users/{id}', methods: ['GET'])]
    public function show(int $id): ?User
    {
        return $this->repository->find($id);
    }

    #[Route('/users', methods: ['POST'])]
    public function store(array $data): User
    {
        $user = new User(
            name: $data['name'],
            email: $data['email'],
        );
        $this->repository->save($user);
        return $user;
    }
}

// Functions
function add(int $a, int $b): int
{
    return $a + $b;
}

function multiply(float ...$numbers): float
{
    return array_reduce($numbers, fn($carry, $n) => $carry * $n, 1.0);
}

// Arrow functions (PHP 7.4+)
$double = fn(int $x): int => $x * 2;
$isEven = fn(int $x): bool => $x % 2 === 0;

// Closures
$greet = function (string $name) use ($greeting): string {
    return "$greeting, $name!";
};

$multiplier = function (int $factor): callable {
    return fn(int $x) => $x * $factor;
};

// Array functions
$numbers = [1, 2, 3, 4, 5];
$doubled = array_map(fn($n) => $n * 2, $numbers);
$evens = array_filter($numbers, fn($n) => $n % 2 === 0);
$sum = array_reduce($numbers, fn($carry, $n) => $carry + $n, 0);

// Match expression (PHP 8.0+)
function httpStatusText(int $code): string
{
    return match($code) {
        200 => 'OK',
        201 => 'Created',
        400 => 'Bad Request',
        404 => 'Not Found',
        500, 502, 503 => 'Server Error',
        default => 'Unknown',
    };
}

// Null safe operator (PHP 8.0+)
function getUserCity(?User $user): ?string
{
    return $user?->getAddress()?->getCity();
}

// Named arguments (PHP 8.0+)
$user = new User(
    name: 'Alice',
    email: 'alice@example.com',
    role: User::ROLE_ADMIN,
    active: true,
);

// Heredoc and Nowdoc
$html = <<<HTML
<div class="user">
    <h1>{$user->getName()}</h1>
    <p>{$user->getEmail()}</p>
</div>
HTML;

$sql = <<<'SQL'
SELECT * FROM users
WHERE active = 1
ORDER BY created_at DESC
SQL;

// Exception handling
try {
    $user->setEmail('invalid-email');
} catch (InvalidArgumentException $e) {
    echo "Validation error: " . $e->getMessage();
} catch (Exception $e) {
    echo "Error: " . $e->getMessage();
} finally {
    echo "Cleanup complete";
}

// Generators
function range_generator(int $start, int $end): Generator
{
    for ($i = $start; $i <= $end; $i++) {
        yield $i;
    }
}

function fibonacci(): Generator
{
    $a = 0;
    $b = 1;
    while (true) {
        yield $a;
        [$a, $b] = [$b, $a + $b];
    }
}

// Stringable interface (PHP 8.0+)
class Message implements \Stringable
{
    public function __construct(private string $content) {}

    public function __toString(): string
    {
        return $this->content;
    }
}

// Entry point
if (php_sapi_name() === 'cli') {
    echo "Running in CLI mode\n";
    
    // Create user
    $user = new User(
        name: 'Test User',
        email: 'test@example.com',
    );
    
    echo $user->greet(greeting: 'Welcome', formal: true) . "\n";
    echo json_encode($user, JSON_PRETTY_PRINT) . "\n";
}
?>

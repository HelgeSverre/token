<?php

declare(strict_types=1);

namespace App\Billing;

use DateTimeImmutable;
use Generator;
use InvalidArgumentException;
use JsonSerializable;
use RuntimeException;
use Stringable;

/**
 * Order line item states, backed for persistence and API responses.
 */
enum LineItemStatus: string
{
    case Pending = 'pending';
    case Shipped = 'shipped';
    case Refunded = 'refunded';

    public function isFinal(): bool
    {
        return match ($this) {
            self::Refunded => true,
            default => false,
        };
    }
}

enum Currency: string
{
    case USD = 'USD';
    case EUR = 'EUR';
    case NOK = 'NOK';

    public function symbol(): string
    {
        return match ($this) {
            self::USD => '$',
            self::EUR => '€',
            self::NOK => 'kr',
        };
    }
}

interface Discountable
{
    public function applyDiscount(float $percentOff): static;
}

interface Taxable
{
    public function taxAmount(float $rate): Money;
}

/**
 * Immutable value object for money, always tracked in minor units (cents)
 * to avoid floating point drift when totals are summed across many lines.
 */
final readonly class Money implements Stringable
{
    private function __construct(
        public int $cents,
        public Currency $currency,
    ) {}

    public static function fromFloat(float $amount, Currency $currency = Currency::USD): self
    {
        return new self((int) round($amount * 100), $currency);
    }

    public static function zero(Currency $currency = Currency::USD): self
    {
        return new self(0, $currency);
    }

    public function add(self $other): self
    {
        $this->assertSameCurrency($other);
        return new self($this->cents + $other->cents, $this->currency);
    }

    public function multiply(float $factor): self
    {
        return new self((int) round($this->cents * $factor), $this->currency);
    }

    public function percentage(float $percent): self
    {
        return $this->multiply($percent / 100.0);
    }

    private function assertSameCurrency(self $other): void
    {
        if ($this->currency !== $other->currency) {
            throw new InvalidArgumentException(
                "Currency mismatch: {$this->currency->value} vs {$other->currency->value}"
            );
        }
    }

    public function __toString(): string
    {
        return sprintf('%s%s', $this->currency->symbol(), number_format($this->cents / 100, 2));
    }
}

/**
 * A single billable line on an order. Fluent methods return new instances
 * where the change is a value-object transform, and `$this` where identity
 * (the line's own mutable state machine) needs to be preserved.
 */
class LineItem implements Discountable, Taxable, JsonSerializable
{
    private LineItemStatus $status = LineItemStatus::Pending;
    private float $discountPercent = 0.0;

    public function __construct(
        public readonly string $sku,
        public readonly string $description,
        private Money $unitPrice,
        private int $quantity = 1,
    ) {
        if ($quantity < 1) {
            throw new InvalidArgumentException("Quantity must be positive, got {$quantity}");
        }
    }

    public function subtotal(): Money
    {
        return $this->unitPrice
            ->multiply((float) $this->quantity)
            ->percentage(100 - $this->discountPercent);
    }

    public function taxAmount(float $rate): Money
    {
        return $this->subtotal()->percentage($rate * 100);
    }

    public function applyDiscount(float $percentOff): static
    {
        if ($percentOff < 0.0 || $percentOff > 100.0) {
            throw new InvalidArgumentException('Discount must be between 0 and 100');
        }
        $this->discountPercent = $percentOff;
        return $this;
    }

    public function ship(): void
    {
        if ($this->status->isFinal()) {
            throw new RuntimeException("Cannot ship a {$this->status->value} line item");
        }
        $this->status = LineItemStatus::Shipped;
    }

    public function refund(): void
    {
        $this->status = LineItemStatus::Refunded;
    }

    public function status(): LineItemStatus
    {
        return $this->status;
    }

    public function jsonSerialize(): array
    {
        return [
            'sku' => $this->sku,
            'description' => $this->description,
            'quantity' => $this->quantity,
            'unit_price' => (string) $this->unitPrice,
            'subtotal' => (string) $this->subtotal(),
            'status' => $this->status->value,
        ];
    }
}

/**
 * Base type for anything that can accumulate line items and produce a
 * total. Kept abstract because "order" and "quote" diverge on what
 * happens at checkout, but share everything about totals.
 */
abstract class Invoiceable
{
    /** @var LineItem[] */
    protected array $lines = [];

    abstract public function checkout(): Receipt;

    public function addLine(LineItem $line): static
    {
        $this->lines[] = $line;
        return $this;
    }

    public function subtotal(Currency $currency = Currency::USD): Money
    {
        return array_reduce(
            $this->lines,
            fn (Money $carry, LineItem $line) => $carry->add($line->subtotal()),
            Money::zero($currency)
        );
    }

    /** @return LineItem[] */
    public function lines(): array
    {
        return $this->lines;
    }
}

final class Receipt implements JsonSerializable
{
    public function __construct(
        public readonly string $orderId,
        public readonly Money $subtotal,
        public readonly Money $tax,
        public readonly Money $total,
        public readonly DateTimeImmutable $issuedAt,
    ) {}

    public function jsonSerialize(): array
    {
        return [
            'order_id' => $this->orderId,
            'subtotal' => (string) $this->subtotal,
            'tax' => (string) $this->tax,
            'total' => (string) $this->total,
            'issued_at' => $this->issuedAt->format(DATE_ATOM),
        ];
    }
}

final class Order extends Invoiceable
{
    private const TAX_RATE = 0.0825;

    private static int $sequence = 1000;

    public readonly string $id;

    public function __construct(
        public readonly string $customerEmail,
        private readonly Currency $currency = Currency::USD,
    ) {
        $this->id = 'ORD-' . self::$sequence++;
    }

    public function checkout(): Receipt
    {
        if ($this->lines === []) {
            throw new RuntimeException('Cannot check out an empty order');
        }

        $subtotal = $this->subtotal($this->currency);
        $tax = array_reduce(
            $this->lines,
            fn (Money $carry, LineItem $line) => $carry->add($line->taxAmount(self::TAX_RATE)),
            Money::zero($this->currency)
        );

        foreach ($this->lines as $line) {
            $line->ship();
        }

        return new Receipt(
            orderId: $this->id,
            subtotal: $subtotal,
            tax: $tax,
            total: $subtotal->add($tax),
            issuedAt: new DateTimeImmutable(),
        );
    }
}

/**
 * Repository backed by a plain array, standing in for a real persistence
 * layer in this syntax sample.
 */
final class OrderRepository
{
    /** @var array<string, Order> */
    private array $orders = [];

    public function save(Order $order): void
    {
        $this->orders[$order->id] = $order;
    }

    public function find(string $id): ?Order
    {
        return $this->orders[$id] ?? null;
    }

    /** @return Order[] */
    public function forCustomer(string $email): array
    {
        return array_values(array_filter(
            $this->orders,
            fn (Order $order) => $order->customerEmail === $email
        ));
    }
}

/**
 * Pipeline of independent validation/normalization steps, composed with
 * the PHP 8.5 pipe operator so the flow reads top to bottom.
 *
 * @param array{sku: string, description: string, price: float, qty?: int} $raw
 */
function normalizeLineInput(array $raw): array
{
    $trimStrings = static fn (array $row): array => [
        ...$row,
        'sku' => trim($row['sku']),
        'description' => trim($row['description']),
    ];

    $upperSku = static fn (array $row): array => [
        ...$row,
        'sku' => strtoupper($row['sku']),
    ];

    $defaultQty = static fn (array $row): array => [
        ...$row,
        'qty' => max(1, $row['qty'] ?? 1),
    ];

    return $raw
        |> $trimStrings
        |> $upperSku
        |> $defaultQty;
}

function lineItemFromInput(array $normalized): LineItem
{
    return new LineItem(
        sku: $normalized['sku'],
        description: $normalized['description'],
        unitPrice: Money::fromFloat($normalized['price']),
        quantity: $normalized['qty'],
    );
}

/**
 * @param array<int, array{sku: string, description: string, price: float, qty?: int}> $rows
 */
function buildOrderFromRows(string $customerEmail, array $rows): Order
{
    $order = new Order($customerEmail);

    foreach ($rows as $row) {
        $line = normalizeLineInput($row) |> lineItemFromInput(...);
        $order->addLine($line);
    }

    return $order;
}

// First-class callable syntax (PHP 8.1+) combined with the pipe operator
$formatTotal = fn (Money $money): string => (string) $money;
$shout = strtoupper(...);

function moneyToString(Money $money): string
{
    return (string) $money;
}

function describeReceipt(Receipt $receipt): string
{
    return $receipt->total
        |> moneyToString(...)
        |> strtoupper(...);
}

// Match expression driving a small state machine
function nextStatus(LineItemStatus $current, string $action): LineItemStatus
{
    return match (true) {
        $current === LineItemStatus::Pending && $action === 'ship' => LineItemStatus::Shipped,
        $action === 'refund' => LineItemStatus::Refunded,
        default => $current,
    };
}

// Generator for streaming a large export without loading everything into memory
function exportReceipts(OrderRepository $repo, array $ids): Generator
{
    foreach ($ids as $id) {
        $order = $repo->find($id);
        if ($order === null) {
            continue;
        }
        yield $order->id => $order->checkout();
    }
}

// --- Wiring it all together -------------------------------------------

$repository = new OrderRepository();

$order = buildOrderFromRows('alice@example.com', [
    ['sku' => 'wgt-001', 'description' => 'Widget, medium', 'price' => 19.99, 'qty' => 3],
    ['sku' => 'wgt-002', 'description' => 'Widget, large', 'price' => 29.99],
    ['sku' => 'shp-fee', 'description' => 'Shipping & handling', 'price' => 6.50],
]);

$order->lines()[1]->applyDiscount(15.0);

$repository->save($order);

try {
    $receipt = $order->checkout();
    echo describeReceipt($receipt), PHP_EOL;
    echo json_encode($receipt, JSON_PRETTY_PRINT), PHP_EOL;
} catch (RuntimeException $e) {
    fwrite(STDERR, "Checkout failed: {$e->getMessage()}" . PHP_EOL);
    exit(1);
}

foreach (exportReceipts($repository, [$order->id, 'ORD-9999']) as $orderId => $receipt) {
    printf("%s -> %s%s", $orderId, $formatTotal($receipt->total), PHP_EOL);
}

<?php
declare(strict_types=1);

/**
 * Self-contained invoice renderer: PHP + HTML mixed in one file (no
 * includes, no framework). Demonstrates alternative control-flow syntax
 * (`if:`/`endif;`, `foreach:`/`endforeach;`), short echo tags `<?=`, and
 * regular `<?php ... ?>` blocks side by side for highlighting variety.
 */

final class InvoiceLine
{
    public function __construct(
        public readonly string $description,
        public readonly int $quantity,
        public readonly float $unitPrice,
        public readonly float $taxRate = 0.0825,
    ) {}

    public function subtotal(): float
    {
        return $this->quantity * $this->unitPrice;
    }

    public function tax(): float
    {
        return $this->subtotal() * $this->taxRate;
    }

    public function total(): float
    {
        return $this->subtotal() + $this->tax();
    }
}

function money(float $amount, string $currency = 'USD'): string
{
    $symbols = ['USD' => '$', 'EUR' => '€', 'NOK' => 'kr'];
    return ($symbols[$currency] ?? '') . number_format($amount, 2);
}

$invoiceNumber = 'INV-2026-0417';
$issuedAt = new DateTimeImmutable('2026-08-12');
$dueAt = $issuedAt->modify('+14 days');

$billTo = [
    'name' => 'Northwind Traders',
    'address' => '221B Baker Street',
    'city' => 'London',
    'email' => 'ap@northwind.example',
];

/** @var InvoiceLine[] $lines */
$lines = [
    new InvoiceLine('Consulting — architecture review', 8, 145.00),
    new InvoiceLine('Managed hosting (August)', 1, 89.00, taxRate: 0.0),
    new InvoiceLine('Support retainer', 20, 60.00),
];

$subtotal = array_sum(array_map(fn (InvoiceLine $l) => $l->subtotal(), $lines));
$tax = array_sum(array_map(fn (InvoiceLine $l) => $l->tax(), $lines));
$total = $subtotal + $tax;
$isOverdue = $dueAt < new DateTimeImmutable('now') && $total > 0;
?>
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>Invoice <?= htmlspecialchars($invoiceNumber) ?></title>
    <style>
        body { font-family: -apple-system, sans-serif; color: #222; margin: 2rem; }
        table { width: 100%; border-collapse: collapse; margin-top: 1.5rem; }
        th, td { padding: 0.5rem 0.75rem; border-bottom: 1px solid #ddd; text-align: left; }
        th { background: #f4f4f4; }
        td.num, th.num { text-align: right; }
        .totals td { border-bottom: none; }
        .overdue { color: #b00020; font-weight: bold; }
    </style>
</head>
<body>

<header>
    <h1>Invoice <?= htmlspecialchars($invoiceNumber) ?></h1>
    <p>
        Issued: <?= $issuedAt->format('F j, Y') ?><br>
        Due: <span<?php if ($isOverdue): ?> class="overdue"<?php endif; ?>>
            <?= $dueAt->format('F j, Y') ?><?php if ($isOverdue): ?> (overdue)<?php endif; ?>
        </span>
    </p>
</header>

<section>
    <h2>Bill To</h2>
    <address>
        <?= htmlspecialchars($billTo['name']) ?><br>
        <?= htmlspecialchars($billTo['address']) ?><br>
        <?= htmlspecialchars($billTo['city']) ?><br>
        <a href="mailto:<?= htmlspecialchars($billTo['email']) ?>"><?= htmlspecialchars($billTo['email']) ?></a>
    </address>
</section>

<table>
    <thead>
        <tr>
            <th>Description</th>
            <th class="num">Qty</th>
            <th class="num">Unit Price</th>
            <th class="num">Tax</th>
            <th class="num">Line Total</th>
        </tr>
    </thead>
    <tbody>
    <?php if (empty($lines)): ?>
        <tr><td colspan="5">No line items on this invoice.</td></tr>
    <?php else: ?>
        <?php foreach ($lines as $line): ?>
        <tr>
            <td><?= htmlspecialchars($line->description) ?></td>
            <td class="num"><?= $line->quantity ?></td>
            <td class="num"><?= money($line->unitPrice) ?></td>
            <td class="num">
                <?php if ($line->taxRate > 0): ?>
                    <?= money($line->tax()) ?> (<?= (int) round($line->taxRate * 100) ?>%)
                <?php else: ?>
                    &mdash;
                <?php endif; ?>
            </td>
            <td class="num"><?= money($line->total()) ?></td>
        </tr>
        <?php endforeach; ?>
    <?php endif; ?>
    </tbody>
    <tfoot class="totals">
        <tr>
            <td colspan="4" class="num">Subtotal</td>
            <td class="num"><?= money($subtotal) ?></td>
        </tr>
        <tr>
            <td colspan="4" class="num">Tax</td>
            <td class="num"><?= money($tax) ?></td>
        </tr>
        <tr>
            <td colspan="4" class="num"><strong>Total Due</strong></td>
            <td class="num"><strong><?= money($total) ?></strong></td>
        </tr>
    </tfoot>
</table>

<?php if ($isOverdue): ?>
    <p class="overdue">This invoice is past due. Please remit payment immediately.</p>
<?php endif; ?>

<footer>
    <p>
        <?php
        // Non-short block: build the footer note with a couple of
        // intermediate variables, just to mix long-form <?php with <?= above.
        $daysToPay = $issuedAt->diff($dueAt)->days;
        $note = sprintf('Payment terms: net %d days.', $daysToPay);
        ?>
        <?= htmlspecialchars($note) ?>
    </p>
</footer>

</body>
</html>

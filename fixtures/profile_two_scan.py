# Diagnostic representative: two scans and conditional Tool continuations.
quotes = tools.load_quotes(batch_id)
total = 0
for quote in quotes:
    if quote.quantity > 0:
        total += quote.quantity
if total > 0:
    receipt = tools.record_total(batch_id, total)
else:
    receipt = tools.record_total(batch_id, 0)
for quote in quotes:
    if quote.quantity > 0 and quote.unit_price_cents <= price_limit:
        amount = quote.unit_price_cents * quote.quantity
        total += amount
print(total)
print(receipt)

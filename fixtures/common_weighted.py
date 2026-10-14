quotes = await tools.load_quotes(batch_id)
total = 0
for quote in quotes:
    if quote.quantity > 0 and quote.unit_price_cents <= price_limit:
        amount = quote.unit_price_cents * quote.quantity
        total += amount
receipt = await tools.record_total(batch_id, total)
print(receipt)

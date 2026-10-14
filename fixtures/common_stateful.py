quotes = await tools.load_quotes(batch_id)
total = 0
previous = 0
for quote in quotes:
    if quote.quantity > 0 and quote.unit_price_cents <= price_limit and quote.unit_price_cents >= previous:
        amount = quote.unit_price_cents * quote.quantity
        total += amount
        previous = quote.unit_price_cents
receipt = await tools.record_total(batch_id, total)
print(receipt)

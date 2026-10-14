# Bounded F shape: nested predicates, loop-carried state and branch joins.
quotes = tools.load_quotes(batch_id)
total = 0
previous = 0
for quote in quotes:
    if not (quote.quantity <= 0 or (quote.unit_price_cents > price_limit and previous > 0)):
        if (quote.unit_price_cents >= previous and quote.quantity > 1) or not (previous > 0):
            total += quote.unit_price_cents * quote.quantity
        else:
            total -= quote.quantity
    else:
        total += 1
    previous = quote.unit_price_cents
receipt = tools.record_total(batch_id, total)
print(receipt)

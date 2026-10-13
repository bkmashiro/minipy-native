# Bounded task-local output inside a native scan.
quotes = tools.load_quotes(batch_id)
for quote in quotes:
    print(quote.quantity)

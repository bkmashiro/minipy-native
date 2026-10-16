stats = tools.fetch_stats(request_id)
remaining = stats["limit"] - stats["used"]
print(remaining)
print(stats["label"])

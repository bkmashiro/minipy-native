data = tools.search(request_id)
selected = 0
for item in data["results"]:
    if item["score"] > 80:
        selected = item["id"]
        break
print(selected)

state = tools.fetch_state(request_id)
data = tools.read_text("reports/" + state["filename"])
print(data["content"])

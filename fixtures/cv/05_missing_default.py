state = tools.fetch_state(request_id)
print(state.get("owner", "unassigned"))

state = tools.fetch_state(request_id)
if state["status"] == "ready":
    print(f"ready: {state['count']}")
else:
    print("not ready")

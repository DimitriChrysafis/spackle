ROWS = [
    ("id", "name", "email"),
    (1, "ada", "ada@x.io"),
    (2, "grace", "grace@x.io"),
    (3, "linus", "linus@x.io"),
]

def row_for(user_id):
    for row in ROWS[1:]:
        if row[0] == user_id:
            cols = ROWS[0]
            return {cols[i]: row[i] for i in range(len(row) - 1)}
    return None

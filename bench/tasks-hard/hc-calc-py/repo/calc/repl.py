import sys
from .evaluator import evaluate

def main():
    for line in sys.stdin:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        try:
            print(evaluate(line))
        except Exception as e:
            print(f"error: {e}", file=sys.stderr)

if __name__ == "__main__":
    main()

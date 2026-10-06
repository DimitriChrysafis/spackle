import subprocess, sys
out = subprocess.run([sys.executable, "main.py", "Hello World"],
                     capture_output=True, text=True)
assert out.returncode == 0, out.stderr
assert out.stdout.strip() == "hello-world", out.stdout
print("ok")

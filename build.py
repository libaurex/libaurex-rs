import subprocess
import sys
import platform

os_name = platform.system()

if len(sys.argv) <= 1:
    print("No language specified.")
    sys.exit(1)

language = sys.argv[1]

print("Building the library...")
result = subprocess.run(
    [
        "cargo", "build", "--release"
    ], 
    capture_output=True, 
    text=True
)
print("Done.")

import subprocess

lib_file = ''
if os_name == "Windows":
    lib_file = 'libaurex.dll'
elif os_name == "Darwin":
    lib_file = 'libaurex.dylib'
elif os_name == 'Linux':
    lib_file = 'libaurex.so'

print("Generating bindings...")
result = subprocess.run(
    [
        "cargo", "run", "--bin", "uniffi-bindgen", "generate", "--library", f"target/release/{lib_file}", "--language", language, "--out-dir", "out"
    ], 
    capture_output=True, 
    text=True
)
print("Done.")
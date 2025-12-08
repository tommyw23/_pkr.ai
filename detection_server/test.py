# detection_server/test.py
import requests
import os

# Update this to match the actual file you saw
screenshot_path = r"C:\Users\thoma\Downloads\Business\pkr.ai\poker_dataset\images\Screenshot (28).png"

# Check if it exists
if not os.path.exists(screenshot_path):
    print(f"ERROR: File not found at {screenshot_path}")
    print("\nLooking for files in the folder...")
    folder = r"C:\Users\thoma\Downloads\Business\pkr.ai\poker_dataset\images"
    files = [f for f in os.listdir(folder) if f.endswith(('.png', '.jpg', '.jpeg'))]
    print(f"Found {len(files)} image files:")
    for f in files[:5]:
        print(f"  - {f}")
    exit()

print("Testing detection on:", screenshot_path)

with open(screenshot_path, "rb") as f:
    files = {"file": ("screenshot.png", f, "image/png")}
    response = requests.post("http://127.0.0.1:8000/detect", files=files)

print("\nStatus Code:", response.status_code)
print("Response:", response.json())
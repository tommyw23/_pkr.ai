# detection_server/server.py
from fastapi import FastAPI, UploadFile, File
from fastapi.responses import JSONResponse
from roboflow import Roboflow
import os
import traceback

app = FastAPI()

# Initialize Roboflow with your API key
API_KEY = "W5UhxwHzNmkYRNNq8ujv"
rf = Roboflow(api_key=API_KEY)

print("Loading Roboflow workspace...")
project = rf.workspace().project("my-first-project-b9tuf")
print("Loading Roboflow model...")
model = project.version(2).model
print("✅ Model loaded successfully!")

@app.post("/detect")
async def detect_panel(file: UploadFile = File(...)):
    """
    Accepts a poker screenshot and returns the bounding box
    of the center panel (cards, pot, hero info)
    """
    try:
        print(f"\n📥 Received file: {file.filename}, content_type: {file.content_type}")
        
        # Read uploaded image
        img_bytes = await file.read()
        print(f"📊 Image size: {len(img_bytes)} bytes")
        
        # Save temporarily (Roboflow API needs file path)
        temp_path = "temp_screenshot.jpg"
        with open(temp_path, "wb") as f:
            f.write(img_bytes)
        
        print(f"💾 Saved to: {temp_path}")
        print(f"🔍 Running detection...")
        
        # Run detection
        result = model.predict(temp_path, confidence=40).json()
        
        print(f"✅ Detection complete. Found {len(result.get('predictions', []))} predictions")
        
        # Clean up temp file
        if os.path.exists(temp_path):
            os.remove(temp_path)
        
        # Check if panel was found
        if not result.get('predictions'):
            print("⚠️  No panel detected in image")
            return JSONResponse(
                status_code=404,
                content={"error": "No panel detected"}
            )
        
        # Get highest confidence detection
        pred = result['predictions'][0]
        
        # Convert from center coords to top-left coords
        x = int(pred['x'] - pred['width'] / 2)
        y = int(pred['y'] - pred['height'] / 2)
        width = int(pred['width'])
        height = int(pred['height'])
        
        response_data = {
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "confidence": pred['confidence']
        }
        
        print(f"🎯 Returning: {response_data}")
        
        return JSONResponse(content=response_data)
        
    except Exception as e:
        print(f"\n❌ ERROR in detect_panel:")
        print(f"   Error type: {type(e).__name__}")
        print(f"   Error message: {str(e)}")
        print(f"   Traceback:")
        traceback.print_exc()
        
        return JSONResponse(
            status_code=500,
            content={"error": str(e), "type": type(e).__name__}
        )

@app.get("/health")
async def health_check():
    """Simple health check endpoint"""
    return {"status": "ok", "model": "yolov11-poker-panel"}

if __name__ == "__main__":
    import uvicorn
    print("🚀 Starting detection server...")
    uvicorn.run(app, host="127.0.0.1", port=8000)
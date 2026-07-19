import requests
import json

URL = "http://127.0.0.1:8008/cleanup"

def test_endpoint():
    payload = {
        "raw_transcript": "umm umm so yeah, basically I think we should just, you know, refactor the database layer. umm umm",
        "app_context": "slack" # tests the 'chat' tone profile
    }
    
    print(f"Sending payload to {URL}...")
    print(json.dumps(payload, indent=2))
    print("-" * 40)
    
    try:
        response = requests.post(URL, json=payload)
        response.raise_for_status()
        
        data = response.json()
        print("Response received:")
        print(f"Cleaned Text: {data.get('cleaned_text')}")
    except requests.exceptions.RequestException as e:
        print(f"Request failed: {e}")
        if hasattr(e, 'response') and e.response is not None:
            print(f"Server response: {e.response.text}")

if __name__ == "__main__":
    test_endpoint()



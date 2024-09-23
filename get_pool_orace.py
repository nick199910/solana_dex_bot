import requests
import json

url = "https://api.mainnet.orca.so/v1/whirlpool/list"

try:
    response = requests.get(url)
    response.raise_for_status()  # 如果请求失败，这将引发一个异常
    data = response.json()  # 将响应解析为JSON
    
    print(f"Type of the response: {type(data)}")
    
    if isinstance(data, dict):
        print("The response is a dictionary. Here are the keys:")
        for key in data.keys():
            print(f"- {key}")
        
        if 'whirlpools' in data and isinstance(data['whirlpools'], list) and len(data['whirlpools']) > 0:
            print("\nFirst object in the 'whirlpools' list:")
            print(json.dumps(data['whirlpools'][0], indent=2))
        else:
            print("\nNo 'whirlpools' list found or it's empty.")
    
    elif isinstance(data, list):
        if len(data) > 0:
            print("The response is a list. Here's the first item:")
            print(json.dumps(data[0], indent=2))
        else:
            print("The response is an empty list.")
    
    else:
        print(f"The response is of an unexpected type: {type(data)}")
        print("Here's a string representation of the data:")
        print(str(data)[:1000])  # Print first 1000 characters to avoid overwhelming output

except requests.exceptions.RequestException as e:
    print(f"An error occurred while making the request: {e}")
except json.JSONDecodeError as e:
    print(f"An error occurred while parsing the JSON response: {e}")
    print("Here's the raw response content:")
    print(response.text[:1000])  # Print first 1000 characters of the response
except Exception as e:
    print(f"An unexpected error occurred: {e}")
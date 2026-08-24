#!/usr/bin/env python3
"""
Test script to verify datetime is included in historical data
"""

import requests
import json

def test_timestamps():
    # Test the historical data API directly
    base_url = "http://localhost:3000"
    
    try:
        # First authenticate
        login_response = requests.post(f"{base_url}/api/login", 
                                     json={"username": "admin"})
        
        if login_response.status_code == 200:
            token = login_response.json().get("token")
            print("✅ Authentication successful")
            
            # Get historical data with authorization header
            headers = {"Authorization": f"Bearer {token}"}
            
            # Get available symbols first
            symbols_response = requests.get(f"{base_url}/api/historical", headers=headers)
            if symbols_response.status_code == 200:
                symbols_data = symbols_response.json()
                print(f"📊 Available symbols: {len(symbols_data['symbols'])}")
                
                if symbols_data['symbols']:
                    # Get historical data for the first symbol
                    symbol = "ABB"  # Try ABB which was mentioned in server logs
                    print(f"🔍 Testing symbol: {symbol}")
                    
                    # Get historical data without date filters to see recent data
                    history_response = requests.get(
                        f"{base_url}/api/historical/{symbol}?limit=30", 
                        headers=headers
                    )
                    
                    if history_response.status_code == 200:
                        history_data = history_response.json()
                        print(f"✅ Retrieved {len(history_data['data'])} records")
                        
                        # Check if datetime is included
                        if history_data['data']:
                            first_record = history_data['data'][0]
                            print(f"📋 First record keys: {list(first_record.keys())}")
                            
                            if 'datetime' in first_record:
                                print(f"✅ DATETIME FOUND: {first_record['datetime']}")
                                print(f"💰 Close: {first_record['close']}")
                                print(f"📊 Volume: {first_record['volume']}")
                                
                                # Show more records to see datetime variety
                                print(f"\n📋 Sample records with datetime:")
                                for i, record in enumerate(history_data['data'][:15]):
                                    print(f"  {i+1}. Datetime: {record['datetime']} | Close: {record['close']}")
                                
                                return True
                            else:
                                print("❌ DATETIME NOT FOUND in response")
                                print(f"Record structure: {json.dumps(first_record, indent=2)}")
                                return False
                        else:
                            print("❌ No data records found")
                            return False
                    else:
                        print(f"❌ Failed to get historical data: {history_response.status_code}")
                        print(history_response.text)
                        return False
                else:
                    print("❌ No symbols available")
                    return False
            else:
                print(f"❌ Failed to get symbols: {symbols_response.status_code}")
                print(symbols_response.text)
                return False
        else:
            print(f"❌ Authentication failed: {login_response.status_code}")
            print(login_response.text)
            return False
            
    except Exception as e:
        print(f"❌ Error: {e}")
        return False

if __name__ == "__main__":
    print("=" * 60)
    print("TESTING DATETIME IN HISTORICAL DATA")
    print("=" * 60)
    
    success = test_timestamps()
    
    print("\n" + "=" * 60)
    if success:
        print("🎉 SUCCESS: Datetime is included in historical data!")
    else:
        print("❌ FAILED: Datetime is not working properly")
    print("=" * 60) 
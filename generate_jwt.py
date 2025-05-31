#!/usr/bin/env python3
"""
JWT Token Generator for NSE Socket Server
Generate JWT tokens for testing the WebSocket authentication
"""

import jwt
import time
import uuid
import json
from datetime import datetime, timedelta

# JWT Configuration (must match server)
JWT_SECRET = "your-secret-key-change-in-production"
ALGORITHM = "HS256"

def generate_jwt_token(user_id, permissions=None, expires_in_minutes=60):
    """Generate a JWT token with the specified claims"""
    if permissions is None:
        permissions = ["read_data", "websocket_connect"]
    
    now = datetime.utcnow()
    exp = now + timedelta(minutes=expires_in_minutes)
    
    # JWT Claims
    claims = {
        "sub": user_id,                    # Subject (user_id)
        "jti": str(uuid.uuid4()),          # JWT ID (unique session identifier)
        "exp": int(exp.timestamp()),       # Expiration time
        "iat": int(now.timestamp()),       # Issued at
        "user_id": user_id,                # User identifier
        "permissions": permissions         # User permissions
    }
    
    # Generate token
    token = jwt.encode(claims, JWT_SECRET, algorithm=ALGORITHM)
    
    return token, claims

def generate_test_tokens():
    """Generate test tokens for different scenarios"""
    
    print("JWT Token Generator for NSE Socket Server")
    print("=" * 50)
    
    # Test Token 1: Valid user with standard permissions
    token1, claims1 = generate_jwt_token("user123", ["read_data", "websocket_connect"], 60)
    print(f"\n1. Valid Token (60 min expiry) - User: {claims1['user_id']}")
    print(f"   JWT ID: {claims1['jti']}")
    print(f"   Token: {token1}")
    
    # Test Token 2: Another valid user (to test concurrent connections)
    token2, claims2 = generate_jwt_token("user456", ["read_data", "websocket_connect"], 30)
    print(f"\n2. Another Valid Token (30 min expiry) - User: {claims2['user_id']}")
    print(f"   JWT ID: {claims2['jti']}")
    print(f"   Token: {token2}")
    
    # Test Token 3: Same user as token1 (should conflict if token1 is active)
    token3, claims3 = generate_jwt_token("user123", ["read_data", "websocket_connect"], 45)
    print(f"\n3. Same User Different Session - User: {claims3['user_id']}")
    print(f"   JWT ID: {claims3['jti']}")
    print(f"   Token: {token3}")
    
    # Test Token 4: Expired token (for testing expiration)
    token4, claims4 = generate_jwt_token("user789", ["read_data"], -5)  # Expired 5 minutes ago
    print(f"\n4. Expired Token - User: {claims4['user_id']}")
    print(f"   JWT ID: {claims4['jti']}")
    print(f"   Token: {token4}")
    print(f"   Status: EXPIRED")
    
    # Test Token 5: Admin user with extended permissions
    token5, claims5 = generate_jwt_token("admin", ["read_data", "websocket_connect", "admin"], 120)
    print(f"\n5. Admin Token (120 min expiry) - User: {claims5['user_id']}")
    print(f"   JWT ID: {claims5['jti']}")
    print(f"   Token: {token5}")
    
    print("\n" + "=" * 50)
    print("Testing Instructions:")
    print("1. Start the Rust WebSocket server")
    print("2. Use these tokens in the Authorization header: 'Bearer <token>'")
    print("3. Test concurrent connections with different tokens")
    print("4. Try connecting twice with the same token (should get 409 Conflict)")
    print("5. Test with expired token (should get 401 Unauthorized)")
    
    # Generate curl commands for easy testing
    print("\n" + "=" * 50)
    print("Example curl commands:")
    print(f'curl -H "Authorization: Bearer {token1}" --websocket ws://localhost:8080')
    print(f'curl -H "Authorization: Bearer {token2}" --websocket ws://localhost:8080')
    
    # Save tokens to file for easy access
    tokens_data = {
        "valid_user1": {"token": token1, "claims": claims1},
        "valid_user2": {"token": token2, "claims": claims2},
        "same_user_different_session": {"token": token3, "claims": claims3},
        "expired_token": {"token": token4, "claims": claims4},
        "admin_user": {"token": token5, "claims": claims5}
    }
    
    with open("test_tokens.json", "w") as f:
        json.dump(tokens_data, f, indent=2, default=str)
    
    print(f"\nTokens saved to: test_tokens.json")

def verify_token(token):
    """Verify a JWT token"""
    try:
        decoded = jwt.decode(token, JWT_SECRET, algorithms=[ALGORITHM])
        print("Token is valid!")
        print(f"Claims: {json.dumps(decoded, indent=2)}")
        
        # Check expiration
        exp = decoded.get('exp', 0)
        now = time.time()
        if exp < now:
            print("⚠️  Token is EXPIRED")
        else:
            remaining = exp - now
            print(f"✅ Token expires in {remaining/60:.1f} minutes")
            
        return decoded
    except jwt.ExpiredSignatureError:
        print("❌ Token is expired")
    except jwt.InvalidTokenError as e:
        print(f"❌ Token is invalid: {e}")
    return None

if __name__ == "__main__":
    import sys
    
    if len(sys.argv) > 1:
        if sys.argv[1] == "verify":
            if len(sys.argv) > 2:
                print("Verifying token...")
                verify_token(sys.argv[2])
            else:
                print("Usage: python generate_jwt.py verify <token>")
        else:
            print("Usage: python generate_jwt.py [verify <token>]")
    else:
        generate_test_tokens() 
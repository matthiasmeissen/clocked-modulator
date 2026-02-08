
# Serial to TouchDesigner

## Serial DAT Setup 
- Create a Serial DAT.
- Set Port to your Pico (e.g., COM3 or /dev/tty.usbmodem...).
- In the Connect tab and set Row/Callback Format to One Per Byte.

## Python Parser
- Open the Serial Callbacks DAT.
- Create a Constant CHOP named `constant1`.
- Add 4 channels: chan1, chan2, chan3, chan4.


## Adjust Serial Callback DAT

```
import struct

# Global buffer to hold incoming fragments
raw_buffer = bytearray()

def onReceive(dat, rowIndex, message, bytes, peer):
    global raw_buffer
    
    # 1. Add new bytes to our buffer
    raw_buffer.extend(bytes)
    
    # Define Packet Size: 2 Header + (4 floats * 4 bytes) = 18 bytes
    packet_size = 18
    
    # 2. Process the buffer while we have enough data
    while len(raw_buffer) >= packet_size:
        
        # 3. Check for the Header (0xAA, 0xBB)
        # These correspond to 170, 187 in decimal
        if raw_buffer[0] == 0xAA and raw_buffer[1] == 0xBB:
            
            # 4. Extract the Payload (Bytes 2 to 18)
            payload = raw_buffer[2:packet_size]
            
            try:
                # 5. Unpack Binary to Floats
                # '<' = Little Endian (Standard for Pico)
                # '4f' = 4 Floats
                values = struct.unpack('<4f', payload)
                
                # 6. Send to Constant CHOP
                t = op('constant1')
                t.par.value0 = values[0]
                t.par.value1 = values[1]
                t.par.value2 = values[2]
                t.par.value3 = values[3]
                
            except Exception as e:
                print(f"Parse Error: {e}")

            # 7. Remove this packet from the buffer
            del raw_buffer[0:packet_size]
            
        else:
            # Header not found at start? 
            # Delete the first byte and shift the window to search again.
            del raw_buffer[0]
            
    return
```

## Add CHOP Execute to any value generating CHOP

```
def onValueChange(channel, sampleIndex, val, prev):
	import struct
    
    # 1. Define the Header for BPM ('B')
	header = b'B'
    
    # 2. Pack the Float value
    # '<' = Little Endian (Standard for ARM/Pico)
    # 'f' = float (4 bytes)
	payload = struct.pack('<f', val)
    
    # 3. Combine
	message = header + payload
    
    # 4. Send to Serial DAT
    # Change 'serial1' to whatever your Serial DAT is named
	op('serial1').sendBytes(message)
    
	return
```

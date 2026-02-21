import sys
import time

from pubsub import pub

import meshtastic
import meshtastic.tcp_interface

# simple arg check
if len(sys.argv) < 2:
    print(f"usage: {sys.argv[0]} host")
    sys.exit(1)


def onReceive(packet, interface):  # pylint: disable=unused-argument
    """called when a packet arrives"""
    print(f"Received: {packet}")


def onConnection(interface, topic=pub.AUTO_TOPIC):  # pylint: disable=unused-argument
    """called when we (re)connect to the radio"""
    # defaults to broadcast, specify a destination ID if you wish
    # interface.sendText("hello mesh")
    print("Conected")


pub.subscribe(onReceive, "meshtastic.receive")
pub.subscribe(onConnection, "meshtastic.connection.established")
try:
    iface = meshtastic.meshtastic.serial_interface.SerialInterface()
    while True:
        time.sleep(1000)
    iface.close()
except Exception as ex:
    print(f"Error: Could not connect to {sys.argv[1]} {ex}")
    sys.exit(1)

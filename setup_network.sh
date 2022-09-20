#!/usr/bin/env bash

# Create namespaces
sudo ip net add public
sudo ip net add private

# Create dummy devices
sudo ip -n public l add dummy type dummy
sudo ip -n private l add dummy type dummy

# Set IP addresses
sudo ip -n public a add 10.10.10.10 dev dummy
sudo ip -n private a add 172.20.0.2 dev dummy

# Bring links up
sudo ip -n public l set lo up
sudo ip -n public l set dummy up
sudo ip -n private l set lo up
sudo ip -n private l set dummy up

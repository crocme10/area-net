#!/usr/bin/env bash

echo "Starting Bob..."
./target/release/area-net -c ./config  -p bob -s '"network.controller.d2"=true' 1>/dev/null 2>&1 &
bob_pid=$!
sleep 1
echo "Starting Alice..."
./target/release/area-net -c ./config  -p alice 1>/dev/null 2>&1 &
alice_pid=$!
sleep 1
echo "Starting Dave..."
./target/release/area-net -c ./config  -p dave 1>/dev/null 2>&1 &
dave_pid=$!
sleep 1
echo "Starting Carol..."
./target/release/area-net -c ./config  -p carol 1>/dev/null 2>&1 &
carol_pid=$!
sleep 1
echo "Starting Erin..."
./target/release/area-net -c ./config  -p erin 1>/dev/null 2>&1 &
erin_pid=$!
echo "Done"

echo "Press Ctrl-[x] to stop peer starting with letter [x]"
echo "or press Ctrl-q to stop all peers and terminate"
while : ; do
  read -n 1 key <&1
  if [[ $key = a ]] ; then
    kill -s 0 $alice_pid > /dev/null 2>&1 && { echo "Stopping Alice"; kill -9 $alice_pid > /dev/null 2>&1; }
  elif [[ $key = b ]] ; then
    kill -s 0 $bob_pid > /dev/null 2>&1 && { echo "Stopping Bob"; kill -9 $bob_pid > /dev/null 2>&1; }
  elif [[ $key = c ]] ; then
    kill -s 0 $carol_pid > /dev/null 2>&1 && { echo "Stopping Carol"; kill -9 $carol_pid > /dev/null 2>&1; }
  elif [[ $key = d ]] ; then
    kill -s 0 $dave_pid > /dev/null 2>&1 && { echo "Stopping Dave"; kill -9 $dave_pid > /dev/null 2>&1; }
  elif [[ $key = e ]] ; then
    kill -s 0 $erin_pid > /dev/null 2>&1 && { echo "Stopping Erin"; kill -9 $erin_pid > /dev/null 2>&1; }
  elif [[ $key = q ]] ; then
    echo "Terminate outstanding profiles"
    kill -s 0 $alice_pid > /dev/null 2>&1 && { echo "Stopping Alice"; kill -9 $alice_pid > /dev/null 2>&1; }
    kill -s 0 $bob_pid > /dev/null 2>&1 && { echo "Stopping Bob"; kill -9 $bob_pid > /dev/null 2>&1; }
    kill -s 0 $carol_pid > /dev/null 2>&1 && { echo "Stopping Carol"; kill -9 $carol_pid > /dev/null 2>&1; }
    kill -s 0 $dave_pid > /dev/null 2>&1 && { echo "Stopping Dave"; kill -9 $dave_pid > /dev/null 2>&1; }
    kill -s 0 $erin_pid > /dev/null 2>&1 && { echo "Stopping Erin"; kill -9 $erin_pid > /dev/null 2>&1; }
    break
  else
    echo "Unknown profile"
  fi
done



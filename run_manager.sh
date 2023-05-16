#!/bin/sh
FILE=/services/leader_file.txt

function start_dht_to_mqtt() {
  echo "Starting DHT-TO-MQTT ..."
  /dht-to-mqtt &

}

function stop_dht_to_mqtt() {

  cont=0
  while pgrep -f /dht-to-mqtt > /dev/null 2>&1; do

    force=""

    if [ $cont -gt 9 ]; then
      echo "Forcing kill /dht-to-mqtt "
      force="-9"
    fi

    ppid=$(ps | grep dht-to-mqtt | awk '{print $1}' | head -n 1)

    kill $force $ppid

    if [ $? -eq 0 ]; then  # il processo esisteva ed è stato killato
      echo "Killed dht-to-mqtt "
    fi

    echo "Waiting for dht-to-mqtt death ..."

    sleep 1
    cont=$((cont+1))
  done

}


function check() {

  if [[ -f $FILE ]] ; then

     # il file esiste

     if ! pgrep -f /dht-to-mqtt > /dev/null 2>&1 ; then
       start_dht_to_mqtt
     fi


  else

     # il file non esiste
     stop_dht_to_mqtt

  fi

}

trap "echo 'Ricevuto Signal SIGUSR1'; check" SIGUSR1

while true;
  do
   check
   sleep 5 &
   wait $!
done

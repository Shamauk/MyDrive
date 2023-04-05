## Installation

`./install.sh`
This script will install everything necessary to start running MyDrive.

## Running

`./run.sh`
This will open MyDrive in a new tmux session

## Setting up single board computer

### Material

Rocks64
SD Card
Hard Drive
Powered SATA to USB

### Turotial

1. Install a linux [ISO](https://www.armbian.com/rock64/)
2. Use (balena etcher)[https://www.balena.io/etcher] to create bootable on SD card
3. Setup hardware
   - Insert SD card into Rocks64
   - Plug hard drive into usb port using powered SATA to USB
     - It is imperative you get a powered SATA to USB as the board does not have voltage to power a spinning disk
   - Connect board directly to router or a switch using ethernet
4. Turn on rocks64
5. Go to router address (usually 192.168.0.1 or 192.168.1.1 or 192.168.1.0)
6. View connected devices and look for rocks64 and note the IP address
7. Go to LAN DHCP reservations and reserve rocks64 to that IP address (so it does not change on reload)
8. Go to Port Forwarding and open a port of your choosing for private port 2001 and the local ip address you have reserved
   - Do note if anything else is on port 2001 you can change it here and update the Rocket.toml file
9. Connect to the SBC using ssh
   - ssh root@ipaddress
   - passwd: 1234
10. Change password of root
    - sudo passwd root
11. Find hard drive name
    - sudo fdisk -l /dev/sd?
    - name will be of form /dev/sdX
12. Format disk
    - sudo mkfs.ext4 /dev/sdX
13. Create directory to mount hard drive
    - sudo mkdir -m 1777 /media/hd1
14. Get UUID
    - sudo blkid /dev/sdX
15. Add disk to auto mount
    - sudo pico /etc/fstab
    - Add the following at the end of the file
      - UUID=f5779d66-be6b-4304-ac03-cd47c7f3eab6 /media/hd1 ext4 nofail,defaults 0 0
      - Replace UUID with what you got from 14
16. Reboot
17. Run setup
    - ./install.sh
    - Set port to private port placed in DHCP reservation (e.g 2001)
    - Set directory for files as /media/hd1 (or your mount location)
18. ./run.sh

cp PIN_MAP /mnt/.

/home/devices/iliad/debug.tim
#sleep 1
/home/devices/iliad/debug.tim

/home/devices/iliad/PowerOn

/mnt/bin/linux_load_vectors.elf vectors/iliada0_ilall_burnin_scan_burnin_v0_20240829DG.seq vectors/iliada0_ilall_burnin_scan_burnin_v0_20240829DG.hex


#sleep 1

/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#sleep 1

/home/devices/iliad/PowerOff
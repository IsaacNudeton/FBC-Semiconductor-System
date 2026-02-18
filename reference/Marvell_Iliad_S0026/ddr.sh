cp PIN_MAP /mnt/.

/home/devices/iliad/debug.tim
#sleep 1
/home/devices/iliad/debug.tim

/home/devices/iliad/PowerOn

/mnt/bin/linux_load_vectors.elf vectors/iliada0_ilcq_ddr_dssall_burnin_pad_3200mts_v0_20240903DG.seq vectors/iliada0_ilcq_ddr_dssall_burnin_pad_3200mts_v0_20240903DG.hex


#sleep 1

/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#sleep 1

/home/devices/iliad/PowerOff
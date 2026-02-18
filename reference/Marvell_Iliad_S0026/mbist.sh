cp PIN_MAP /mnt/.

/home/devices/iliad/mbist.tim
#sleep 1
/home/devices/iliad/mbist.tim

/home/devices/iliad/PowerOn

/mnt/bin/linux_load_vectors.elf vectors/iliada0_ilcq_mbist_mbist_burnin_r500s500t500d500cc500cd500_v0_20240930DG.seq vectors/iliada0_ilcq_mbist_mbist_burnin_r500s500t500d500cc500cd500_v0_20240930DG.hex


#sleep 1

/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#sleep 1

/home/devices/iliad/PowerOff
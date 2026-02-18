cp PIN_MAP /mnt/.

/home/devices/iliad/debug.tim
/home/devices/iliad/debug.tim
sleep 1
/home/devices/iliad/PowerOn
sleep 1
/mnt/bin/linux_load_vectors.elf vectors/iliada0_ilall_burnin_scan_burnin_v0_20240829DG.seq vectors/iliada0_ilall_burnin_scan_burnin_v0_20240829DG.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
/mnt/bin/linux_load_vectors.elf vectors/iliada0_ilall_burnin_gpio_toggle_v0_20240802DG.seq vectors/iliada0_ilall_burnin_gpio_toggle_v0_20240802DG.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
/mnt/bin/linux_load_vectors.elf vectors/iliada0_ilcq_mbist_mbist_burnin_r500s500t500d500cc500cd500_v0_20240930DG.seq vectors/iliada0_ilcq_mbist_mbist_burnin_r500s500t500d500cc500cd500_v0_20240930DG.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
/mnt/bin/linux_load_vectors.elf vectors/iliada0_ilcq_ddr_dssall_burnin_pad_3200mts_v0_20240903DG.seq vectors/iliada0_ilcq_ddr_dssall_burnin_pad_3200mts_v0_20240903DG.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
/mnt/bin/linux_load_vectors.elf vectors/iliada0_ilall_comphy_gserp_all_elb_gen5_burnin_v0_20240910DG.seq vectors/iliada0_ilall_comphy_gserp_all_elb_gen5_burnin_v0_20240910DG.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0

/home/devices/iliad/PowerOff
cp PIN_MAP /mnt/.

/home/devices/normandy/debug.tim
/home/devices/normandy/debug.tim

/home/devices/normandy/NominalPwr

### SCAN RESET
/mnt/bin/linux_load_vectors.elf vectors/p_reset_fc__infra_reset__type_htol_ssn.seq vectors/p_reset_fc__infra_reset__type_htol_ssn.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
/mnt/bin/linux_load_vectors.elf vectors/p_reset_fc__infra_reset__type_htol_ssn__hsp_scan_mode_entry.seq vectors/p_reset_fc__infra_reset__type_htol_ssn__hsp_scan_mode_entry.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
/mnt/bin/linux_load_vectors.elf vectors/p_reset_fc__infra_reset__type_htol_ssn__scan_mode_secure.seq vectors/p_reset_fc__infra_reset__type_htol_ssn__scan_mode_secure.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0

#/home/devices/normandy/temp.sh

### SCAN
/mnt/bin/linux_load_vectors.elf vectors/p_htol_fc__all__htol_burnin_pr_test_setup.seq vectors/p_htol_fc__all__htol_burnin_pr_test_setup.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
/mnt/bin/linux_load_vectors.elf vectors/p_htol_fc__all__htol_burnin_pr_ssn_setup.seq vectors/p_htol_fc__all__htol_burnin_pr_ssn_setup.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
/mnt/bin/linux_load_vectors.elf vectors/p_htol_fc__all__htol_burnin_pr_payload_first.seq vectors/p_htol_fc__all__htol_burnin_pr_payload_first.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
/mnt/bin/linux_load_vectors.elf vectors/p_htol_fc__all__htol_burnin_pr_payload.seq vectors/p_htol_fc__all__htol_burnin_pr_payload.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
/home/devices/normandy/temp.sh

#/home/devices/normandy/PowerOff
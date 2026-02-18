cp PIN_MAP /mnt/.

/home/devices/normandy/debug.tim
/home/devices/normandy/debug.tim

/home/devices/normandy/NominalPwr

### RESET
/mnt/bin/linux_load_vectors.elf vectors/p_reset_fc__infra_reset__type_htol.seq vectors/p_reset_fc__infra_reset__type_htol.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0

#/home/devices/normandy/temp.sh

### MBIST
#/mnt/bin/linux_load_vectors.elf vectors/p_mbist_fc__vconst__htol.seq vectors/p_mbist_fc__vconst__htol.hex
#/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
#/mnt/bin/linux_load_vectors.elf vectors/p_mbist_fc__vconst_veng_vram__htol.seq vectors/p_mbist_fc__vconst_veng_vram__htol.hex
#/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
#/mnt/bin/linux_load_vectors.elf vectors/p_mbist_fc__vconst_veng_vram_vhbmd__htol.seq vectors/p_mbist_fc__vconst_veng_vram_vhbmd__htol.hex
#/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
#/mnt/bin/linux_load_vectors.elf vectors/p_mbist_fc__veng_vram__htol.seq vectors/p_mbist_fc__veng_vram__htol.hex
#/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
#/mnt/bin/linux_load_vectors.elf vectors/p_mbist_fc__vhbmd__htol.seq vectors/p_mbist_fc__vhbmd__htol.hex
#/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
 
### PCIE
#/mnt/bin/linux_load_vectors.elf vectors/p_pcie_sc_n__test_pcie_burnin_htol.seq vectors/p_pcie_sc_n__test_pcie_burnin_htol.hex
#/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh

### HBM
/mnt/bin/linux_load_vectors.elf vectors/p_hbmio_fc__hbm012345_allch__ilbio_htol_04g_sk_combo.seq vectors/p_hbmio_fc__hbm012345_allch__ilbio_htol_04g_sk_combo.hex
/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
/home/devices/normandy/temp.sh

### ANC
#/mnt/bin/linux_load_vectors.elf vectors/p_anc_fc__allss__anc_phy_setup_burnin.seq vectors/p_anc_fc__allss__anc_phy_setup_burnin.hex
#/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
#/mnt/bin/linux_load_vectors.elf vectors/p_anc_fc__allss__ate_burnin_int_lb_nes_1p25g_eth_nrz.seq vectors/p_anc_fc__allss__ate_burnin_int_lb_nes_1p25g_eth_nrz.hex
#/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
#/mnt/bin/linux_load_vectors.elf vectors/p_anc_fc__allss__ate_eth_pup_cmn_burnin.seq vectors/p_anc_fc__allss__ate_eth_pup_cmn_burnin.hex
#/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh
#/mnt/bin/linux_load_vectors.elf vectors/p_anc_fc__allss__eth_burnin.seq vectors/p_anc_fc__allss__eth_burnin.hex
#/mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
#/home/devices/normandy/temp.sh

#/home/devices/normandy/PowerOff
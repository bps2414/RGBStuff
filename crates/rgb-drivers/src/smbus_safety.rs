use rgb_core::DriverError;

/// Allowed SMBus 7-bit addresses for ENE DRAM RGB controllers on Asgard DDR4 modules.
pub const ENE_DRAM_MIN_ADDR: u8 = 0x70;
pub const ENE_DRAM_MAX_ADDR: u8 = 0x77;

/// Reserved SPD EEPROM range (`0x50..=0x57`) that MUST NEVER be written.
pub const SPD_EEPROM_MIN_ADDR: u8 = 0x50;
pub const SPD_EEPROM_MAX_ADDR: u8 = 0x57;

/// Validates that an SMBus target address is strictly within the safe ENE DRAM RGB whitelist
/// (`0x70..=0x77`) and never touches SPD EEPROM (`0x50..=0x57`) or arbitrary I2C targets.
pub fn validate_smbus_address(addr: u8) -> Result<u8, DriverError> {
    if (SPD_EEPROM_MIN_ADDR..=SPD_EEPROM_MAX_ADDR).contains(&addr) {
        return Err(DriverError::SmbusSafety(format!(
            "blocked write to SPD EEPROM range 0x{addr:02X}"
        )));
    }
    if !(ENE_DRAM_MIN_ADDR..=ENE_DRAM_MAX_ADDR).contains(&addr) {
        return Err(DriverError::SmbusSafety(format!(
            "address 0x{addr:02X} outside allowed ENE DRAM whitelist 0x70..=0x77"
        )));
    }
    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spd_eeprom_addresses_are_always_blocked() {
        for addr in 0x50..=0x57 {
            let err = validate_smbus_address(addr).expect_err("SPD must be rejected");
            assert!(matches!(err, DriverError::SmbusSafety(_)));
        }
    }

    #[test]
    fn non_whitelist_addresses_are_blocked() {
        for addr in [0x00, 0x18, 0x2C, 0x4E, 0x6F, 0x78, 0x7F] {
            assert!(validate_smbus_address(addr).is_err());
        }
    }

    #[test]
    fn ene_dram_whitelist_is_allowed() {
        for addr in 0x70..=0x77 {
            assert_eq!(validate_smbus_address(addr).unwrap(), addr);
        }
    }
}

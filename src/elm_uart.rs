use core::fmt::{Debug, Formatter};
use cortex_m::prelude::_embedded_hal_serial_Read;
use embassy_rp::peripherals::UART0;
use embassy_rp::uart;
use embassy_rp::uart::{Blocking, BufferedUart, Uart};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, WithTimeout};
use embedded_io_async::{Read, Write};
use static_cell::StaticCell;
use crate::{elm_commands, ElmUart, ToMainEvents, Irqs, INCOMING_EVENT_CHANNEL};
use crate::errors::{ToRustAGaugeError, ToRustAGaugeErrorSeverity, ToRustAGaugeErrorWithSeverity};

const LOCAL_RX_BUFFER_LEN: usize = 256;
const UART_TIMEOUT: Duration = Duration::from_millis(10000u64);

const DELIMITER_U8: u8 = '>' as u8;

#[embassy_executor::task]
pub async fn elm_uart_task(r: ElmUart){
    let sender: Sender<CriticalSectionRawMutex, ToMainEvents, 10> = INCOMING_EVENT_CHANNEL.sender();


    let mut uart_config = uart::Config::default();
    uart_config.baudrate = 115200;

    let mut uart = uart::Uart::new_blocking(r.uart0, r.tx_pin, r.rx_pin, uart_config);
    
    let mut rx_buf = SizedUartBuffer{
        buffer: [0u8; LOCAL_RX_BUFFER_LEN],
        end: 0,
    };
    
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ELM_RESET.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_ECHO.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ENABLE_HEADERS.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::SET_PROTOCOL_5.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::SET_TIMEOUT_64.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::EntirelyRecoverable).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_SPACES.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_MEMORY.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ENABLE_AUTO_TIMINGS_1.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::EntirelyRecoverable).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::SET_CUSTOM_HEADERS.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;
    

}
async fn result_unpacker<'a, T, E>(result: Result<T, E>, 
                         sender: Sender<'a, CriticalSectionRawMutex, ToMainEvents, 10>,
                         error_severity: ToRustAGaugeErrorSeverity
) -> Option<T> 
where E: Into<ToRustAGaugeError>
{
    match result{
        Ok(v) => {
            Some(v)
        }
        Err(e) => {
            let error = ToRustAGaugeErrorWithSeverity::from_with_severity(e, error_severity);
            sender.send(ToMainEvents::ElmError(error)).await;
            None
        }
    }
}

pub struct SizedUartBuffer{
    buffer: [u8; LOCAL_RX_BUFFER_LEN],
    end: usize,
}

impl Debug for SizedUartBuffer {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        let used_slice = &self.buffer[..self.end];
        write!(f, "{:?}", used_slice)
    }
}


/// reads from Uart until it finds the delimiter. Could read past the delimiter, but in this use case
/// there are no unprompted writes to uart 
/// 
/// # Arguments 
/// 
/// * `uart`: 
/// * `delimiter_char`: 
/// * `buffer`:
/// 
/// returns: Result<()>, ToRustAGaugeError>
/// 
/// 
/// # Examples 
/// 
/// ```
/// 
/// ```
async fn uart_read_until_char<'a>(uart: &mut Uart<'a, UART0, Blocking>,
                                  delimiter: u8,
                                  rx_buffer: &mut SizedUartBuffer
) -> Result<(), ToRustAGaugeError>{

    let mut index: usize = 0;
    rx_buffer.end = 0;
    
    let mut is_delimiter_found: bool = false;
    
    while !is_delimiter_found && index < LOCAL_RX_BUFFER_LEN {
        
        let temp_slice = &mut rx_buffer.buffer[index..];
        
        match uart.read(){
            Ok(word) => { // timeout OK( UartRead OK( length read ) ) 
                if temp_slice[..len].contains(&delimiter){
                    is_delimiter_found = true;
                }
                index = index + len;
            }
            Err(e) => { // timeout OK( UartRead Err( UartError ) ) 
                return Err(ToRustAGaugeError::UartError(e));
            }
        }
    }
    
    if is_delimiter_found {
        rx_buffer.end = index;
        return Ok(())
    }
    
    Err(ToRustAGaugeError::UartBufferOverflowError())
}

async fn uart_write_read<'a>(uart: &mut BufferedUart<'a, UART0>, 
                             message: &[u8], 
                             rx_buffer: &mut SizedUartBuffer
) -> Result<(), ToRustAGaugeError>{
    uart.write(message).with_timeout(UART_TIMEOUT).await??;
    uart.blocking_flush()?;
    defmt::info!("`uart_write_read` wrote: {:?}", message);
    embassy_time::block_for(Duration::from_millis(100));
    let result = uart_read_until_char(uart, DELIMITER_U8, rx_buffer).await?;
    defmt::info!("`uart_write_read` read: {:?}", result);
    Ok(())
    
}

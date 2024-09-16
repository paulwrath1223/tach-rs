use core::fmt::{Debug, Formatter};
use embassy_rp::peripherals::UART0;
use embassy_rp::uart;
use embassy_rp::uart::BufferedUart;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, WithTimeout};
use embedded_io_async::{Read, ReadReady, Write};
use static_cell::StaticCell;
use crate::{elm_commands, ElmUart, ToMainEvents, Irqs, INCOMING_EVENT_CHANNEL};
use crate::errors::{ToRustAGaugeError, ToRustAGaugeErrorSeverity, ToRustAGaugeErrorWithSeverity};

const LOCAL_RX_BUFFER_LEN: usize = 256;
const UART_TIMEOUT: Duration = Duration::from_millis(1000u64);

const DELIMITER_U8: u8 = '>' as u8;

#[embassy_executor::task]
pub async fn elm_uart_task(r: ElmUart){
    let sender: Sender<CriticalSectionRawMutex, ToMainEvents, 10> = INCOMING_EVENT_CHANNEL.sender();
    let (tx_pin, rx_pin, uart_resource) = (r.tx_pin, r.rx_pin, r.uart0);
    static TX_BUF: StaticCell<[u8; 16]> = StaticCell::new();
    let tx_buf = &mut TX_BUF.init([0; 16])[..];
    static RX_BUF: StaticCell<[u8; 16]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; 16])[..];

    let mut uart_config = uart::Config::default();
    uart_config.baudrate = 115200;

    let mut uart = BufferedUart::new(uart_resource, Irqs, tx_pin, rx_pin, tx_buf, rx_buf, uart_config);
    
    let mut rx_buf = SizedUartBuffer{
        buffer: [0u8; LOCAL_RX_BUFFER_LEN],
        end: 0,
    };

    defmt::info!("sending {:?} ({:?})", elm_commands::ELM_RESET, elm_commands::ELM_RESET.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ELM_RESET.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;

    defmt::info!("sending {:?} ({:?})", elm_commands::DISABLE_ECHO, elm_commands::DISABLE_ECHO.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_ECHO.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;

    defmt::info!("sending {:?} ({:?})", elm_commands::ENABLE_HEADERS, elm_commands::ENABLE_HEADERS.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ENABLE_HEADERS.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;

    defmt::info!("sending {:?} ({:?})", elm_commands::SET_PROTOCOL_5, elm_commands::SET_PROTOCOL_5.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::SET_PROTOCOL_5.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;

    defmt::info!("sending {:?} ({:?})", elm_commands::SET_TIMEOUT_64, elm_commands::SET_TIMEOUT_64.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::SET_TIMEOUT_64.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::EntirelyRecoverable).await;

    defmt::info!("sending {:?} ({:?})", elm_commands::DISABLE_SPACES, elm_commands::DISABLE_SPACES.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_SPACES.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;

    defmt::info!("sending {:?} ({:?})", elm_commands::DISABLE_MEMORY, elm_commands::DISABLE_MEMORY.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_MEMORY.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;

    defmt::info!("sending {:?} ({:?})", elm_commands::ENABLE_AUTO_TIMINGS_1, elm_commands::ENABLE_AUTO_TIMINGS_1.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ENABLE_AUTO_TIMINGS_1.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::EntirelyRecoverable).await;

    defmt::info!("sending {:?} ({:?})", elm_commands::SET_CUSTOM_HEADERS, elm_commands::SET_CUSTOM_HEADERS.as_bytes());
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

impl defmt::Format for SizedUartBuffer{
    fn format(&self, f: defmt::Formatter<'_>) {
        let used_slice = &self.buffer[0..self.end];
        defmt::write!(f, "{:?}", used_slice)
    }
}

impl Debug for SizedUartBuffer {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        let used_slice = &self.buffer[0..self.end];
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
async fn uart_read_until_char<'a>(uart: &mut BufferedUart<'a, UART0>,
                                  delimiter: u8,
                                  rx_buffer: &mut SizedUartBuffer
) -> Result<(), ToRustAGaugeError>{

    let mut index: usize = 0;
    rx_buffer.end = 0;
    
    while index < LOCAL_RX_BUFFER_LEN {
        
        let temp_slice = &mut rx_buffer.buffer[index..];
        
        match uart.read(temp_slice).with_timeout(UART_TIMEOUT).await{
            Ok(Ok(len)) => { // timeout OK( UartRead OK( length read ) ) 
                if temp_slice[..len].contains(&delimiter){
                    rx_buffer.end = index + len;
                    return Ok(())
                }
                index = index + len;
            }
            Ok(Err(e)) => { // timeout OK( UartRead Err( UartError ) ) 
                return Err(ToRustAGaugeError::UartError(e));
            }
            Err(e) => {
                return Err(ToRustAGaugeError::UartTimeoutError(e));
            }
        }
    }

    Err(ToRustAGaugeError::UartBufferOverflowError())
}

async fn clear_read_buf<'a>(uart: &mut BufferedUart<'a, UART0>,
                            rx_buffer: &mut SizedUartBuffer
) -> Result<(), ToRustAGaugeError> {
    while uart.read_ready()?{
        uart.read(&mut rx_buffer.buffer).await?;
    }
    Ok(())
}



async fn uart_write_read<'a>(uart: &mut BufferedUart<'a, UART0>,
                             message: &[u8], 
                             rx_buffer: &mut SizedUartBuffer
) -> Result<(), ToRustAGaugeError>{
    clear_read_buf(uart, rx_buffer).await?;
    defmt::info!("`uart_write_read` writing: {:?}", message);
    uart.blocking_write(message)?;
    uart.blocking_flush()?;
    defmt::info!("`uart_write_read` wrote and flushed: {:?}", message);
    embassy_time::block_for(Duration::from_millis(20));
    uart_read_until_char(uart, DELIMITER_U8, rx_buffer).await?;
    defmt::info!("`uart_write_read` read: {:?}", rx_buffer);
    Ok(())
}

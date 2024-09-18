use core::marker::PhantomData;
use embassy_rp::peripherals::UART0;
use embassy_rp::uart;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, WithTimeout};
use crate::{elm_commands, ElmUart, ToMainEvents, Irqs, INCOMING_EVENT_CHANNEL};
use crate::byte_parsing::{CharByte, SizedUartBuffer};
use crate::errors::{ToRustAGaugeError, ToRustAGaugeErrorSeverity, ToRustAGaugeErrorWithSeverity};

pub(crate) const LOCAL_RX_BUFFER_LEN: usize = 256;
const UART_TIMEOUT: Duration = Duration::from_millis(1000u64);

const DELIMITER_U8: u8 = '>' as u8;

#[embassy_executor::task]
pub async fn elm_uart_task(r: ElmUart){
    let sender: Sender<CriticalSectionRawMutex, ToMainEvents, 10> = INCOMING_EVENT_CHANNEL.sender();

    let mut uart_config = uart::Config::default();
    uart_config.baudrate = 115200;

    let mut uart = embassy_rp::uart::Uart::new(r.uart0, r.tx_pin, r.rx_pin, Irqs, r.dma0, r.dma1, uart_config);
    
    let mut rx_buf: SizedUartBuffer<CharByte> = SizedUartBuffer{
        buffer: [0u8; LOCAL_RX_BUFFER_LEN],
        end: 0,
        phantom: PhantomData,
    };

    // defmt::info!("sending {:?} ({:?})", elm_commands::ELM_RESET, elm_commands::ELM_RESET.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ELM_RESET.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;

    // defmt::info!("sending {:?} ({:?})", elm_commands::DISABLE_ECHO, elm_commands::DISABLE_ECHO.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_ECHO.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;

    // defmt::info!("sending {:?} ({:?})", elm_commands::ENABLE_HEADERS, elm_commands::ENABLE_HEADERS.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ENABLE_HEADERS.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;

    // defmt::info!("sending {:?} ({:?})", elm_commands::SET_PROTOCOL_5, elm_commands::SET_PROTOCOL_5.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::SET_PROTOCOL_5.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;

    // defmt::info!("sending {:?} ({:?})", elm_commands::SET_TIMEOUT_64, elm_commands::SET_TIMEOUT_64.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::SET_TIMEOUT_64.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::EntirelyRecoverable).await;

    // defmt::info!("sending {:?} ({:?})", elm_commands::DISABLE_SPACES, elm_commands::DISABLE_SPACES.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_SPACES.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;

    // defmt::info!("sending {:?} ({:?})", elm_commands::DISABLE_MEMORY, elm_commands::DISABLE_MEMORY.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_MEMORY.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;

    // defmt::info!("sending {:?} ({:?})", elm_commands::ENABLE_AUTO_TIMINGS_1, elm_commands::ENABLE_AUTO_TIMINGS_1.as_bytes());
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ENABLE_AUTO_TIMINGS_1.as_bytes(), &mut rx_buf
    ).await, sender, ToRustAGaugeErrorSeverity::EntirelyRecoverable).await;

    // defmt::info!("sending {:?} ({:?})", elm_commands::SET_CUSTOM_HEADERS, elm_commands::SET_CUSTOM_HEADERS.as_bytes());
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
async fn uart_read_until_char<'a>(uart: &mut uart::Uart<'a, UART0, uart::Async>,
                                  delimiter: u8,
                                  rx_buffer: &mut SizedUartBuffer<CharByte>
) -> Result<(), ToRustAGaugeError>{
    rx_buffer.end = 0;

    let mut temp_buffer: [u8; 1] = [0u8];
    
    while rx_buffer.end < LOCAL_RX_BUFFER_LEN {
        
        match uart.read(&mut temp_buffer).with_timeout(UART_TIMEOUT).await{
            Ok(Ok(_)) => { // timeout OK( UartRead OK( length read ) )
                if temp_buffer[0] == delimiter{
                    return Ok(())
                }
                rx_buffer.add_element(temp_buffer[0]);
                // `add_element` returns false for failures, 
                // but the only failure mode is a full buffer, 
                // which is already explicitly check for in this function. 
                // Therefore, it can be safely ignored
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



async fn uart_write_read<'a>(uart: &mut uart::Uart<'a, UART0, uart::Async>,
                             message: &[u8], 
                             rx_buffer: &mut SizedUartBuffer<CharByte>
) -> Result<(), ToRustAGaugeError>{
    uart.blocking_write(message)?;
    uart.blocking_flush()?;
    defmt::info!("`uart_write_read` wrote and flushed: {:?}", message);
    embassy_time::block_for(Duration::from_millis(20));
    uart_read_until_char(uart, DELIMITER_U8, rx_buffer).await?;
    defmt::info!("`uart_write_read` read: {:?}", rx_buffer);
    Ok(())
}
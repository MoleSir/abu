# abu-provider

API integration supporting multiple vendors.


## Features

- [x] chat
  - [x] anthropic
  - [x] openai
  - [x] deepseek   
- [x] embed 
  - [x] openai


## Examples

````rust
dotenv::from_filename("./env/openai.env")?;
let openai = OpenAi::from_env()?;
let request = ChatRequestBuilder::default()
    .model("deepseek-chat" )
    .messages([
        ChatMessage::user("hi!"),
    ])
    .build()?;
        
let response = openai.chat(&request).await?;
println!("{:#?}", response);
````
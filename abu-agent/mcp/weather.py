import requests
from dotenv import load_dotenv
from fastmcp import FastMCP
import os

mcp = FastMCP("weather")

load_dotenv()

API_KEY = os.getenv('OPENWEATHERMAP_API_KEY') 
if not API_KEY:
    raise RuntimeError("OPENWEATHERMAP_API_KEY 未设置")
BASE_URL = "http://api.openweathermap.org/data/2.5/weather"


@mcp.tool()
def get_weather(location: str) -> str:
    """获取指定地点的天气预报。
    参数：
        location (str): 城市名，如 'Beijing'。
    返回：
        str: 天气信息。
    """
    try:
        # 发送API请求
        params = {
            "q": location,
            "appid": API_KEY,
            "units": "metric",
            "lang": "zh_cn"
        }
        response = requests.get(BASE_URL, params=params)
        response.raise_for_status()

        # 解析返回数据
        weather_data = response.json()
        temp = weather_data["main"]["temp"]
        description = weather_data["weather"][0]["description"]
        humidity = weather_data["main"]["humidity"]

        return f"{location}的天气：温度 {temp}°C，{description}，湿度{humidity}%"

    except requests.RequestException as e:
        return f"获取天气信息失败：{str(e)}"


if __name__ == "__main__":
    mcp.run()
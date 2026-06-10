from local_speak_input.postprocess.punctuation import AutoPunctuator


def test_chinese_question_mark():
    punctuator = AutoPunctuator()

    assert punctuator.punctuate("明天可以吗") == "明天可以吗？"


def test_english_period():
    punctuator = AutoPunctuator()

    assert punctuator.punctuate("hello world") == "hello world."
